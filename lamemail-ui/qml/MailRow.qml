import QtQuick
import Lamemail

Item {
    id: row

    required property int index
    required property string account
    required property string accountColor
    required property string icon
    required property bool dense
    required property bool dim
    required property bool strike
    required property bool italic
    required property int indent
    required property string styleColor
    required property string sender
    required property string recipient
    required property int moreRecipients
    required property string otherRecipients
    required property string subject
    required property string time
    required property bool seen
    required property bool flagged
    required property bool answered
    required property bool forwarded
    required property bool fresh
    required property int attachments
    required property string path
    required property string kind
    property bool locked: false
    property bool waiting: false
    readonly property color tint: Theme[accountColor] ?? Theme.accent
    readonly property color iconTint: dim ? Theme.muted : (Theme[styleColor] ?? Theme.muted)
    readonly property bool current: ListView.isCurrentItem
    readonly property color textColor: dim ? Theme.muted
                                           : (seen ? Qt.tint(Theme.muted, Qt.alpha(Theme.foreground, 0.35)) : Theme.foregroundBright)
    readonly property int textSize: dense ? Theme.fontSize : Theme.fontSize + 2
    signal clicked()
    signal starClicked()

    width: ListView.view ? ListView.view.width : 0
    height: dense ? 40 : 58
    scale: tap.pressed ? 0.995 : 1
    Behavior on scale { NumberAnimation { duration: 80 } }

    Rectangle {
        anchors.fill: parent
        color: Theme.foreground
        opacity: hover.hovered || row.current ? 0.08 : (row.seen ? 0 : 0.04)
        Behavior on opacity { NumberAnimation { duration: 120 } }
    }

    Rectangle {
        id: glow
        anchors.fill: parent
        color: Theme.accent
        opacity: 0

        SequentialAnimation {
            id: glowAnimation
            NumberAnimation { target: glow; property: "opacity"; to: 0.22; duration: 150 }
            NumberAnimation { target: glow; property: "opacity"; to: 0; duration: 700; easing.type: Easing.OutCubic }
        }
    }

    Component.onCompleted: if (fresh) glowAnimation.start()
    onFlaggedChanged: star.pop()

    Item {
        id: iconBox
        anchors.left: parent.left
        anchors.leftMargin: 24 + row.indent * 16
        anchors.verticalCenter: parent.verticalCenter
        width: 36
        height: 36

        Rectangle {
            id: tile
            anchors.centerIn: parent
            width: row.dense ? 26 : 36
            height: width
            radius: Theme.radius
            color: Qt.alpha(row.iconTint, row.dim ? 0.12 : 0.2)

            Icon {
                id: glyph
                anchors.centerIn: parent
                name: row.locked || row.waiting ? "sync" : row.icon
                size: row.dense ? 15 : 21
                color: row.iconTint

                RotationAnimator on rotation {
                    running: row.locked || row.waiting
                    from: 0
                    to: 360
                    duration: 1000
                    loops: Animation.Infinite
                    onRunningChanged: if (!running) glyph.rotation = 0
                }
            }
        }

        Rectangle {
            readonly property real diameter: row.dense ? 9 : 12
            x: tile.x + tile.width - diameter * 0.7
            y: tile.y - diameter * 0.3
            width: diameter
            height: diameter
            radius: diameter / 2
            color: row.dim ? Theme.muted : Theme.accent
            border.width: 2
            border.color: Theme.background
            scale: row.seen ? 0 : 1
            Behavior on scale { NumberAnimation { duration: 180; easing.type: Easing.OutBack } }
        }
    }

    Item {
        id: content
        anchors.left: iconBox.right
        anchors.right: parent.right
        anchors.leftMargin: 20
        anchors.rightMargin: 24
        height: parent.height

        Column {
            id: lines
            anchors.left: parent.left
            anchors.right: star.left
            anchors.rightMargin: 8
            anchors.verticalCenter: parent.verticalCenter
            spacing: 2

            Row {
                id: firstLine
                width: parent.width
                spacing: 8

                Tag {
                    id: tag
                    anchors.verticalCenter: parent.verticalCenter
                    opacity: row.seen ? 0.6 : 1
                    text: row.account
                    tint: row.tint
                }

                Text {
                    readonly property string people: row.recipient.length > 0 ? row.sender + "&nbsp;&nbsp;\u2192&nbsp;&nbsp;" + row.recipient : row.sender
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.min(implicitWidth, parent.width - tag.width - parent.spacing - (more.visible ? more.width + parent.spacing : 0))
                    textFormat: Text.StyledText
                    text: (row.dense ? row.sender + "&nbsp;&nbsp;\u00b7&nbsp;&nbsp;<b>" + row.subject.replace(/&/g, "&amp;").replace(/</g, "&lt;") + "</b>" : people)
                          .replace(/\{dim\}/g, Qt.alpha(row.textColor, 0.5))
                          .replace(/<\/?b>/g, row.seen || row.dim ? "" : "$&")
                    color: row.textColor
                    font.family: Theme.fontFamily
                    font.pixelSize: row.textSize
                    font.italic: row.italic
                    font.strikeout: row.strike
                    elide: Text.ElideRight
                }

                Text {
                    id: more
                    anchors.verticalCenter: parent.verticalCenter
                    visible: row.moreRecipients > 0 && !row.dense
                    text: "+" + row.moreRecipients
                    color: Theme.accent
                    font.family: Theme.fontFamily
                    font.pixelSize: row.textSize

                    HoverHandler {
                        id: moreHover
                    }

                    Tip {
                        visible: moreHover.hovered
                        text: row.otherRecipients
                    }
                }
            }

            Text {
                visible: !row.dense
                width: parent.width
                text: row.subject.length > 0 ? row.subject : "—"
                color: row.textColor
                font.family: Theme.fontFamily
                font.pixelSize: row.textSize
                font.italic: row.italic
                font.strikeout: row.strike
                elide: Text.ElideRight
            }
        }

        Grid {
            id: meta
            anchors.right: parent.right
            anchors.top: lines.top
            anchors.topMargin: (firstLine.height - timeText.height) / 2
            width: Math.max(implicitWidth, widestTime.advanceWidth)
            columns: row.dense ? 2 : 1
            layoutDirection: Qt.RightToLeft
            columnSpacing: 12
            rowSpacing: 6
            verticalItemAlignment: Grid.AlignVCenter

            TextMetrics {
                id: widestTime
                font: timeText.font
                text: "00.00.0000"
            }

            Text {
                id: timeText
                text: row.time
                color: row.seen || row.dim ? Theme.muted : Theme.accent
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize
            }

            Row {
                spacing: 8

                Icon {
                    visible: row.attachments > 0
                    name: "attachment"
                    size: 13
                    color: Theme.muted
                }

                Icon {
                    visible: row.answered
                    name: "reply"
                    size: 13
                    color: Theme.muted
                }

                Icon {
                    visible: row.forwarded
                    name: "forward"
                    size: 13
                    color: Theme.muted
                }
            }
        }

        IconButton {
            id: star
            anchors.right: meta.left
            anchors.rightMargin: 8
            anchors.verticalCenter: parent.verticalCenter
            name: "star"
            iconSize: row.dense ? 15 : 18
            filled: row.flagged
            tip: row.flagged ? Strings.unstarTip : Strings.starTip
            iconColor: row.flagged ? Theme.yellow : (star.hovered ? Theme.foreground : Theme.muted)
            opacity: row.flagged || star.hovered ? 1 : (hover.hovered ? 0.6 : 0)
            Behavior on opacity { NumberAnimation { duration: 120 } }
            onClicked: row.starClicked()
        }
    }

    Rule {
        inset: 24
    }

    HoverHandler {
        id: hover
        cursorShape: row.locked ? Qt.ArrowCursor : Qt.PointingHandCursor
    }

    TapHandler {
        id: tap
        enabled: !row.locked
        onTapped: row.clicked()
    }
}
