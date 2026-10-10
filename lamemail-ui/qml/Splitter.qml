import QtQuick
import Lamemail

MouseArea {
    id: splitter
    property bool vertical: true
    signal dragged(point position)

    cursorShape: vertical ? Qt.SplitHCursor : Qt.SplitVCursor
    hoverEnabled: true
    preventStealing: true
    onPositionChanged: (mouse) => {
        if (pressed)
            splitter.dragged(mapToItem(parent, mouse.x, mouse.y))
    }

    Rectangle {
        anchors.centerIn: parent
        width: splitter.vertical ? 2 : parent.width
        height: splitter.vertical ? parent.height : 2
        color: Theme.accent
        opacity: splitter.pressed ? 0.8 : (splitter.containsMouse ? 0.5 : 0)
        Behavior on opacity { NumberAnimation { duration: 120 } }
    }
}
