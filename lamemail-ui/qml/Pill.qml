import QtQuick
import Lamemail

Rectangle {
    id: pill
    property string text
    property string icon
    property color swatch: "transparent"
    property bool on: false
    property bool outlined: false
    property string tip
    signal clicked()

    width: pill.text.length > 0 ? content.implicitWidth + 20 : height
    height: 30
    radius: Theme.radius
    color: Qt.alpha(on ? Theme.accent : Theme.foreground, on ? 0.22 : (hover.hovered ? 0.12 : 0.06))
    border.width: 1
    border.color: outlined ? Theme.accent : "transparent"
    Behavior on color { ColorAnimation { duration: 120 } }

    Row {
        id: content
        anchors.centerIn: parent
        spacing: 6

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            visible: pill.swatch.a > 0
            width: 14
            height: 14
            radius: 3
            color: pill.swatch
        }

        Icon {
            anchors.verticalCenter: parent.verticalCenter
            visible: pill.icon.length > 0
            name: pill.icon
            size: 14
            color: pill.on ? Theme.accent : Theme.muted
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: text.length > 0
            text: pill.text
            color: pill.on ? Theme.foregroundBright : Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize - 1
        }
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }

    Tip {
        visible: hover.hovered && pill.tip.length > 0
        text: pill.tip
    }

    TapHandler {
        onTapped: pill.clicked()
    }
}
