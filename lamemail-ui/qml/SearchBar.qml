import QtQuick
import Lamemail

Item {
    id: bar
    property bool open: false
    property string hits
    signal searched(string text)
    signal listRequested()

    function show() {
        bar.open = true
        field.forceActiveFocus()
        field.selectAll()
    }

    function hide() {
        bar.open = false
        field.clear()
        delay.stop()
        bar.searched("")
    }

    height: open ? 56 : 0
    visible: height > 0
    clip: true
    opacity: open ? 1 : 0
    enabled: open
    Behavior on height { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
    Behavior on opacity { NumberAnimation { duration: 150 } }

    SearchInput {
        id: field
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.topMargin: 12
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        rightPadding: bar.hits.length > 0 ? hitsText.width + 22 : 10
        placeholderText: Strings.searchMail
        onTextChanged: delay.restart()
        Keys.onPressed: (event) => {
            if (event.key === Qt.Key_Escape) {
                bar.hide()
                bar.listRequested()
            } else if (event.key === Qt.Key_Down || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                bar.listRequested()
            } else {
                return
            }
            event.accepted = true
        }

        Text {
            id: hitsText
            anchors.right: parent.right
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            text: bar.hits
            color: Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize - 1
        }
    }

    Timer {
        id: delay
        interval: 200
        onTriggered: bar.searched(field.text)
    }

    Rule {}
}
