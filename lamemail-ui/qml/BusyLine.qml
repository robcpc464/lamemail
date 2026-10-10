import QtQuick
import Lamemail

Item {
    id: line
    property bool running: false

    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    height: 2
    clip: true
    visible: running

    Rectangle {
        id: runner
        width: parent.width * 0.25
        height: parent.height
        color: Theme.accent
        SequentialAnimation on x {
            running: line.running
            loops: Animation.Infinite
            NumberAnimation { from: -runner.width; to: line.width; duration: 1200; easing.type: Easing.InOutQuad }
        }
    }
}
