import QtQuick
import Lamemail

Column {
    id: empty
    property string error
    property bool searching: false
    property string kind

    width: Math.min(560, parent ? parent.width - 48 : 560)
    spacing: 14

    Icon {
        anchors.horizontalCenter: parent.horizontalCenter
        name: empty.error.length > 0 ? "mail"
              : empty.searching ? "search"
              : empty.kind.length > 0 ? Styles.kindIcon(empty.kind)
              : "mail"
        size: 56
        color: Theme.muted
        opacity: 0.5
    }

    Text {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        text: empty.error.length > 0 ? Strings.noAccounts
              : empty.searching ? Strings.noResults
              : empty.kind.length > 0 ? Strings.emptyKind.replace("{}", Styles.kindLabel(empty.kind))
              : Strings.empty
        color: Theme.muted
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize + 1
        wrapMode: Text.Wrap
    }

    Rectangle {
        visible: empty.error.length > 0
        anchors.horizontalCenter: parent.horizontalCenter
        width: commands.implicitWidth + 32
        height: commands.implicitHeight + 24
        radius: Theme.radius
        color: Theme.surface
        opacity: 0.8

        Text {
            id: commands
            anchors.centerIn: parent
            text: Strings.commands
            color: Theme.foreground
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize
            lineHeight: 1.4
        }
    }

    Text {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        text: Strings.shortcuts
        color: Theme.muted
        opacity: 0.7
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize - 1
        wrapMode: Text.Wrap
    }
}
