import QtCore
import QtQuick
import Lamemail

Item {
    id: split
    required property string category
    property real defaultFirstWidth: 0.6
    property real defaultSecondHeight: 0.3
    property real minimumSecondHeight: 0.15
    property real maximumSecondHeight: 0.7
    default property alias first: firstPane.data
    property alias second: secondPane.data
    readonly property bool wide: width >= 1300
    property real mix: wide ? 1 : 0
    readonly property real secondWidth: mix * width * (1 - layout.firstWidth) + (1 - mix) * width
    readonly property real secondHeight: mix * height + (1 - mix) * height * layout.secondHeight

    Behavior on mix { NumberAnimation { duration: 260; easing.type: Easing.OutCubic } }

    Settings {
        id: layout
        category: split.category
        property real firstWidth: split.defaultFirstWidth
        property real secondHeight: split.defaultSecondHeight
    }

    Item {
        id: firstPane
        width: split.width - split.mix * split.secondWidth
        height: split.height - (1 - split.mix) * split.secondHeight
    }

    Item {
        id: secondPane
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        width: split.secondWidth
        height: split.secondHeight
    }

    Item {
        anchors.fill: secondPane

        Rectangle {
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            height: 1
            color: Theme.line
            opacity: 0.25 * (1 - split.mix)
        }

        Rectangle {
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            width: 1
            color: Theme.line
            opacity: 0.25 * split.mix
        }
    }

    Splitter {
        x: split.width * layout.firstWidth - width / 2
        width: 10
        height: split.height
        visible: split.mix === 1
        onDragged: (position) => layout.firstWidth = Math.max(0.3, Math.min(0.8, position.x / split.width))
    }

    Splitter {
        vertical: false
        y: split.height - split.secondHeight - height / 2
        width: split.width
        height: 10
        visible: split.mix === 0
        onDragged: (position) => layout.secondHeight = Math.max(split.minimumSecondHeight,
            Math.min(split.maximumSecondHeight, (split.height - position.y) / split.height))
    }
}
