import QtQuick
import Lamemail

Item {
    id: row

    required property string name
    required property string address
    required property string account
    required property string accountColor
    required property string time
    required property bool written
    property bool chosen: false
    property real tagWidth: 0
    readonly property color tint: Theme[accountColor] ?? Theme.accent
    readonly property bool current: ListView.isCurrentItem
    signal picked(bool cc)

    onChosenChanged: if (chosen) pop.start()

    width: ListView.view ? ListView.view.width : 0
    height: 52

    Rectangle {
        anchors.fill: parent
        color: Theme.foreground
        opacity: area.containsMouse || row.current ? 0.08 : 0
        Behavior on opacity { NumberAnimation { duration: 120 } }
    }

    Text {
        id: title
        anchors.left: parent.left
        anchors.leftMargin: 20
        anchors.right: check.left
        anchors.rightMargin: 12
        anchors.bottom: parent.verticalCenter
        text: row.name.length > 0 ? row.name : row.address
        color: row.written ? Theme.foregroundBright : Theme.foreground
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize + 1
        font.bold: row.written
        elide: Text.ElideRight
    }

    Text {
        anchors.left: title.left
        anchors.right: check.left
        anchors.rightMargin: 12
        anchors.top: parent.verticalCenter
        anchors.topMargin: 2
        visible: row.name.length > 0
        text: row.address
        color: Theme.muted
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize - 1
        elide: Text.ElideRight
    }

    Text {
        id: when
        anchors.right: parent.right
        anchors.rightMargin: 20
        anchors.baseline: title.baseline
        text: row.time
        color: Theme.muted
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize - 1
    }

    TextMetrics {
        id: widestTime
        font: when.font
        text: "00.00.0000"
    }

    Icon {
        id: check
        anchors.right: parent.right
        anchors.rightMargin: 20 + Math.max(widestTime.advanceWidth, row.tagWidth, tag.width) + 16
        anchors.verticalCenter: parent.verticalCenter
        opacity: row.chosen ? 1 : 0
        name: "check"
        size: 18
        color: Theme.accent
        Behavior on opacity { NumberAnimation { duration: 120 } }

        SequentialAnimation {
            id: pop
            NumberAnimation { target: check; property: "scale"; to: 1.3; duration: 110; easing.type: Easing.OutQuad }
            NumberAnimation { target: check; property: "scale"; to: 1; duration: 240; easing.type: Easing.OutBack }
        }
    }

    Tag {
        id: tag
        anchors.right: parent.right
        anchors.rightMargin: 20
        anchors.top: parent.verticalCenter
        anchors.topMargin: 3
        text: row.account
        tint: row.tint
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: (mouse) => row.picked((mouse.modifiers & Qt.ControlModifier) !== 0)
    }
}
