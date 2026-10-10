pragma ComponentBehavior: Bound

import QtQuick
import Lamemail

Rectangle {
    id: panel

    property bool running: false
    property string question
    property var report: null
    property bool open: false
    property real maximumHeight: 200
    readonly property bool asking: question.length > 0 && !running
    readonly property bool reporting: !running && !asking && report !== null
    signal answered(string answer)
    signal skipped()
    signal cancelled()
    signal closed()

    function focusAnswer() {
        answer.forceActiveFocus()
    }

    function send() {
        const text = answer.text.trim()
        if (text.length === 0)
            return
        answer.text = ""
        panel.answered(text)
    }

    height: column.height + 12
    radius: Theme.radius
    color: Qt.alpha(Theme.accent, 0.08)
    border.width: 1
    border.color: Qt.alpha(Theme.accent, panel.asking ? 0.6 : 0.25)
    clip: true

    Column {
        id: column
        x: 10
        y: 6
        width: panel.width - 20
        spacing: 8

        Item {
            width: column.width
            height: Math.max(24, headline.implicitHeight)

            Icon {
                id: sparkle
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.topMargin: 5
                name: "sparkles"
                size: 14
                color: Theme.accent

                SequentialAnimation on opacity {
                    running: panel.running
                    loops: Animation.Infinite
                    alwaysRunToEnd: true
                    NumberAnimation { from: 1; to: 0.3; duration: 700; easing.type: Easing.InOutSine }
                    NumberAnimation { from: 0.3; to: 1; duration: 700; easing.type: Easing.InOutSine }
                }
            }

            Text {
                id: headline
                anchors.left: sparkle.right
                anchors.leftMargin: 8
                anchors.right: tools.left
                anchors.rightMargin: 8
                anchors.verticalCenter: parent.verticalCenter
                text: panel.running ? Strings.aiThinking
                                    : panel.asking ? Strings.aiAsks.replace("{}", panel.question)
                                                   : panel.report?.summary ?? ""
                color: panel.reporting ? Theme.foreground : Theme.foregroundBright
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize - 1
                wrapMode: panel.asking ? Text.Wrap : Text.NoWrap
                elide: panel.asking ? Text.ElideNone : Text.ElideRight
            }

            Row {
                id: tools
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: 10

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: !panel.reporting
                    text: Strings.aiStop
                    color: Theme.muted
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontSize - 2
                }

                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: panel.reporting
                    name: panel.open ? "chevronDown" : "chevronRight"
                    size: 14
                    color: Theme.muted
                }

                Icon {
                    id: close
                    anchors.verticalCenter: parent.verticalCenter
                    visible: panel.reporting
                    name: "close"
                    size: 14
                    color: closeHover.hovered ? Theme.foregroundBright : Theme.muted

                    HoverHandler {
                        id: closeHover
                        margin: 6
                        cursorShape: Qt.PointingHandCursor
                    }

                    Tip {
                        visible: closeHover.hovered
                        text: Strings.aiReportHide
                    }

                    TapHandler {
                        margin: 6
                        onTapped: panel.closed()
                    }
                }
            }

            HoverHandler {
                id: headerHover
                enabled: panel.reporting
                cursorShape: Qt.PointingHandCursor
            }

            Tip {
                visible: headerHover.hovered && !closeHover.hovered
                text: Strings.aiReportTip
            }

            TapHandler {
                enabled: panel.reporting
                onTapped: panel.open = !panel.open
            }
        }

        Row {
            visible: panel.asking
            width: column.width
            spacing: 8

            Field {
                id: answer
                width: parent.width - skip.width - parent.spacing
                placeholderText: Strings.aiAnswerPlaceholder
                Keys.onReturnPressed: panel.send()
                Keys.onEnterPressed: panel.send()
                Keys.onEscapePressed: panel.cancelled()
            }

            Pill {
                id: skip
                anchors.verticalCenter: answer.verticalCenter
                text: Strings.aiSkip
                onClicked: panel.skipped()
            }
        }

        Item {
            visible: panel.reporting && panel.open
            width: column.width
            height: Math.min(sections.height, panel.maximumHeight)

            Flickable {
                id: details
                anchors.fill: parent
                contentHeight: sections.height
                clip: true
                boundsBehavior: Flickable.StopAtBounds

                Column {
                    id: sections
                    width: details.width
                    spacing: 10

                    Repeater {
                        model: panel.report?.sections ?? []

                        Column {
                            id: section
                            required property var modelData
                            width: sections.width
                            spacing: 2

                            Text {
                                width: section.width
                                text: section.modelData.title
                                color: Theme.foregroundBright
                                font.family: Theme.fontFamily
                                font.pixelSize: Theme.fontSize - 1
                                font.bold: true
                                elide: Text.ElideRight
                            }

                            Repeater {
                                model: section.modelData.lines

                                Text {
                                    required property string modelData
                                    width: section.width
                                    leftPadding: 10
                                    text: modelData
                                    color: Theme.muted
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontSize - 1
                                    wrapMode: Text.Wrap
                                }
                            }
                        }
                    }
                }
            }

            WheelArea {
                flickable: details
            }
        }
    }

    BusyLine {
        running: panel.running
    }
}
