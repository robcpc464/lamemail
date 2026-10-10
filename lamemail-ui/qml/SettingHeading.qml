import QtQuick
import Lamemail

Column {
    id: heading
    property string title
    property string hint

    width: parent ? parent.width : 0
    topPadding: 32
    bottomPadding: 4
    leftPadding: 24
    rightPadding: 24
    spacing: 4

    Text {
        width: heading.width - heading.leftPadding - heading.rightPadding
        text: heading.title
        color: Theme.foregroundBright
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize + 3
        font.bold: true
        elide: Text.ElideRight
    }

    Text {
        width: heading.width - heading.leftPadding - heading.rightPadding
        visible: text.length > 0
        text: heading.hint
        color: Theme.muted
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize
        wrapMode: Text.Wrap
    }
}
