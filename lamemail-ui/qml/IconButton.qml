import QtQuick
import Lamemail

Item {
    id: button
    property string name: "mail"
    property color iconColor: Theme.muted
    property real iconSize: 20
    property bool filled: false
    property bool spinning: false
    property bool on: false
    property bool danger: false
    property bool dot: false
    property string tip
    readonly property alias hovered: hover.hovered
    signal clicked()

    function pop() {
        popAnimation.start()
    }

    width: 36
    height: 36

    Rectangle {
        anchors.fill: parent
        radius: Theme.radius
        color: button.on ? Theme.accent : Theme.foreground
        opacity: button.on ? 0.2 : (hover.hovered ? 0.1 : 0)
        Behavior on opacity { NumberAnimation { duration: 120 } }
    }

    Icon {
        id: glyph
        anchors.centerIn: parent
        name: button.name
        size: button.iconSize
        color: button.danger && hover.hovered ? Theme.red : button.iconColor
        filled: button.filled
        Behavior on color { ColorAnimation { duration: 200 } }

        RotationAnimator on rotation {
            running: button.spinning
            from: 0
            to: 360
            duration: 1000
            loops: Animation.Infinite
            alwaysRunToEnd: true
        }
    }

    Rectangle {
        visible: button.dot
        x: glyph.x + glyph.width - width / 2
        y: glyph.y - height / 2
        width: 8
        height: 8
        radius: width / 2
        color: Theme.accent
    }

    SequentialAnimation {
        id: popAnimation
        NumberAnimation { target: glyph; property: "scale"; to: 1.3; duration: 110; easing.type: Easing.OutQuad }
        NumberAnimation { target: glyph; property: "scale"; to: 1; duration: 240; easing.type: Easing.OutBack }
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
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
