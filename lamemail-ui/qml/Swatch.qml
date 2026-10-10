import QtQuick
import Lamemail

Rectangle {
    id: swatch
    property string colorName
    property bool chosen: false
    readonly property color shade: Theme[colorName]
    signal clicked()

    width: 30
    height: 30
    radius: 15
    color: "transparent"
    border.width: 2
    border.color: Qt.alpha(shade, chosen ? 1 : 0)
    Behavior on border.color { ColorAnimation { duration: 150 } }

    Rectangle {
        anchors.centerIn: parent
        width: 18
        height: 18
        radius: 9
        color: swatch.shade
        scale: hover.hovered && !swatch.chosen ? 1.2 : 1
        Behavior on scale { NumberAnimation { duration: 120; easing.type: Easing.OutCubic } }
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }

    TapHandler {
        onTapped: if (!swatch.chosen) swatch.clicked()
    }
}
