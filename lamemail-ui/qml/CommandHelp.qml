pragma ComponentBehavior: Bound

import QtQuick
import Lamemail

Item {
    id: help

    required property var modelData

    width: ListView.view ? ListView.view.width : 0
    height: content.height + 36

    Column {
        id: content
        y: 18
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        spacing: 6

        TextEdit {
            width: parent.width
            text: help.modelData.usage
            readOnly: true
            selectByMouse: true
            wrapMode: TextEdit.Wrap
            color: Theme.foregroundBright
            selectionColor: Qt.alpha(Theme.accent, 0.35)
            selectedTextColor: Theme.foregroundBright
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize + 1
            font.bold: true
        }

        Text {
            width: parent.width
            text: help.modelData.about
            color: Theme.foreground
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize
            wrapMode: Text.Wrap
        }

        Repeater {
            model: help.modelData.arguments

            Column {
                id: argument
                required property var modelData

                width: content.width
                topPadding: 4
                spacing: 2

                Text {
                    width: parent.width
                    text: argument.modelData.name
                    color: Theme.accent
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontSize - 1
                    wrapMode: Text.Wrap
                }

                Text {
                    width: parent.width
                    leftPadding: 16
                    visible: text.length > 0
                    text: [argument.modelData.help, (argument.modelData.values ?? []).join(" · ")]
                        .filter(line => line.length > 0).join("\n")
                    color: Theme.muted
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontSize - 1
                    wrapMode: Text.Wrap
                }
            }
        }
    }

    Rule {
        inset: 24
    }
}
