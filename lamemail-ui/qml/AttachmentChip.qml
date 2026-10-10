import QtQuick
import Lamemail

Rectangle {
    id: chip
    property string icon: "attachment"
    property string tip: Strings.open
    property string name
    property string size
    property bool busy: false
    readonly property alias hovered: hover.hovered
    signal clicked()

    width: chip.name.length > 0 ? content.implicitWidth + 24 : height
    height: 28
    radius: Theme.radius
    color: Qt.alpha(Theme.foreground, hover.hovered ? 0.12 : 0.06)
    border.width: 1
    border.color: Qt.alpha(Theme.line, 0.35)
    Behavior on color { ColorAnimation { duration: 120 } }

    Row {
        id: content
        anchors.centerIn: parent
        spacing: 8

        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: chip.icon
            size: 13
            color: chip.busy ? Theme.accent : Theme.muted

            SequentialAnimation on opacity {
                running: chip.busy
                loops: Animation.Infinite
                alwaysRunToEnd: true
                NumberAnimation { to: 0.3; duration: 450; easing.type: Easing.InOutQuad }
                NumberAnimation { to: 1; duration: 450; easing.type: Easing.InOutQuad }
            }
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: text.length > 0
            text: chip.name
            color: Theme.foreground
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize - 1
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: text.length > 0
            text: chip.size
            color: Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize - 1
        }
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }

    Tip {
        visible: hover.hovered
        text: chip.tip
    }

    TapHandler {
        gesturePolicy: TapHandler.WithinBounds
        onTapped: chip.clicked()
    }
}
