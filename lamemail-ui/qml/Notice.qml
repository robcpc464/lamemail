import QtQuick
import Lamemail

Item {
    id: notice
    property string title
    property string text

    width: parent ? parent.width : 0
    height: box.height + 16

    Rectangle {
        id: box
        x: 24
        y: 8
        width: parent.width - 48
        height: content.implicitHeight + 28
        radius: Theme.radius
        color: Qt.alpha(Theme.yellow, 0.1)
        border.width: 1
        border.color: Qt.alpha(Theme.yellow, 0.45)

        Row {
            id: content
            x: 14
            y: 14
            width: parent.width - 28
            spacing: 12

            Icon {
                name: "junk"
                size: 18
                color: Theme.yellow
            }

            Column {
                width: parent.width - 30
                spacing: 4

                Text {
                    text: notice.title
                    color: Theme.foregroundBright
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontSize + 1
                    font.bold: true
                }

                Text {
                    width: parent.width
                    text: notice.text
                    color: Theme.foreground
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontSize
                    wrapMode: Text.Wrap
                    lineHeight: 1.15
                }
            }
        }
    }
}
