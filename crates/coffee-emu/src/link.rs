//! The emulator's side of the lockstep link to an engine process.

use anyhow::{Context, Result, bail};
use coffee_io::wire::{self, MAX_FRAME, Message, PROTOCOL_VERSION};
use coffee_io::{ActuatorFrame, SensorFrame};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub struct EngineLink {
    r: Box<dyn Read + Send>,
    w: Box<dyn Write + Send>,
    child: Option<Child>,
}

pub fn default_engine_path() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    let dir = exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let name = if cfg!(windows) {
        "coffee-engine.exe"
    } else {
        "coffee-engine"
    };
    dir.join(name)
}

impl EngineLink {
    /// Spawn the engine with `--emu stdio` and talk over its pipes.
    pub fn spawn(engine: &PathBuf, extra_args: &[String], quiet: bool) -> Result<Self> {
        let mut cmd = Command::new(engine);
        cmd.arg("--emu").arg("stdio").arg("--http").arg("off");
        cmd.args(extra_args);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped());
        cmd.stderr(if quiet {
            Stdio::null()
        } else {
            Stdio::inherit()
        });
        let mut child = cmd.spawn().with_context(|| {
            format!(
                "spawning engine `{}` (build it with `cargo build -p coffee-engine`)",
                engine.display()
            )
        })?;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut link = Self {
            r: Box::new(BufReader::new(stdout)),
            w: Box::new(BufWriter::new(stdin)),
            child: Some(child),
        };
        link.hello()?;
        Ok(link)
    }

    pub fn from_tcp(s: std::net::TcpStream) -> Result<Self> {
        s.set_nodelay(true)?;
        let mut link = Self {
            r: Box::new(BufReader::new(s.try_clone()?)),
            w: Box::new(BufWriter::new(s)),
            child: None,
        };
        link.hello()?;
        Ok(link)
    }

    /// Talk to an ESP32 running the firmware in `bridge` mode over USB serial.
    pub fn from_serial(path: &str) -> Result<Self> {
        let port = serialport::new(path, 921_600)
            .timeout(std::time::Duration::from_secs(2))
            .open()
            .with_context(|| format!("opening {path}"))?;
        let reader = port.try_clone().context("cloning serial port")?;
        let mut link = Self {
            r: Box::new(BufReader::new(reader)),
            w: Box::new(BufWriter::new(port)),
            child: None,
        };
        link.hello()?;
        Ok(link)
    }

    fn send(&mut self, m: &Message) -> Result<()> {
        let mut buf = [0u8; MAX_FRAME];
        let n = wire::encode(m, &mut buf);
        self.w.write_all(&buf[..n])?;
        self.w.flush()?;
        Ok(())
    }

    fn recv(&mut self) -> Result<Message> {
        let mut hdr = [0u8; 3];
        self.r
            .read_exact(&mut hdr)
            .context("engine closed the link")?;
        let len = u16::from_le_bytes([hdr[0], hdr[1]]) as usize;
        if len > MAX_FRAME - 3 {
            bail!("frame too long");
        }
        let mut buf = [0u8; MAX_FRAME];
        buf[..3].copy_from_slice(&hdr);
        self.r.read_exact(&mut buf[3..3 + len])?;
        Ok(wire::decode(&buf[..3 + len])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?
            .0)
    }

    fn hello(&mut self) -> Result<()> {
        self.send(&Message::Hello {
            version: PROTOCOL_VERSION,
        })?;
        match self.recv()? {
            Message::Hello { version } if version == PROTOCOL_VERSION => Ok(()),
            Message::Hello { version } => {
                bail!("engine speaks protocol {version}, emulator {PROTOCOL_VERSION}")
            }
            m => bail!("expected Hello, got {m:?}"),
        }
    }

    pub fn exchange(&mut self, s: SensorFrame) -> Result<ActuatorFrame> {
        self.send(&Message::Sensors(s))?;
        match self.recv()? {
            Message::Actuators(a) if a.half_cycle == s.half_cycle => Ok(a),
            Message::Actuators(a) => bail!(
                "engine answered half-cycle {} for {}",
                a.half_cycle,
                s.half_cycle
            ),
            m => bail!("expected Actuators, got {m:?}"),
        }
    }

    pub fn close(mut self) {
        let _ = self.send(&Message::Bye);
        if let Some(mut c) = self.child.take() {
            let _ = c.wait();
        }
    }
}

impl Drop for EngineLink {
    fn drop(&mut self) {
        if let Some(c) = self.child.as_mut() {
            let _ = c.kill();
        }
    }
}
