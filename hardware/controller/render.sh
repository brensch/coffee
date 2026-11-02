#!/bin/bash
# render.sh board.kicad_pcb out.png [layers] [width_px]
. ~/opt/kienv.sh
L=${3:-F.Cu,B.Cu,F.SilkS,Edge.Cuts,F.CrtYd}
WPX=${4:-2400}
T=$(mktemp -d -p ${TMPDIR:-/tmp})
kicad-cli pcb export svg "$1" -o $T/b.svg -l $L --page-size-mode 2 --exclude-drawing-sheet >/dev/null 2>&1
echo "<html><body style='margin:0;background:#fff'><img src='b.svg' style='width:${WPX}px'></body></html>" > $T/i.html
H=$(python3 -c "import re;s=open('$T/b.svg').read();m=re.search(r'width=\"([\d.]+)mm\" height=\"([\d.]+)mm\"',s);print(int($WPX*float(m.group(2))/float(m.group(1)))+4)")
google-chrome --headless --no-sandbox --hide-scrollbars --screenshot="$2" --window-size=$WPX,$H file://$T/i.html >/dev/null 2>&1
rm -rf $T
