import QtQuick
import Lamemail

Rectangle {
    id: button

    property string icon
    property string tip
    property bool ready: true
    signal clicked()

    width: 64
    height: 36
    radius: Theme.radius
    color: ready && hover.hovered ? Qt.lighter(Theme.accent, 1.12) : Theme.accent
    opacity: ready ? 1 : 0.4
    Behavior on color { ColorAnimation { duration: 150 } }
    Behavior on opacity { NumberAnimation { duration: 150 } }

    Icon {
        anchors.centerIn: parent
        name: button.icon
        size: 18
        color: Theme.background
    }

    HoverHandler {
        id: hover
        cursorShape: button.ready ? Qt.PointingHandCursor : Qt.ArrowCursor
    }

    Tip {
        visible: hover.hovered && button.tip.length > 0
        text: button.tip
    }

    TapHandler {
        gesturePolicy: TapHandler.WithinBounds
        onTapped: button.clicked()
    }
}
