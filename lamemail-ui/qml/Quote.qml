import QtQuick
import Lamemail

Column {
    id: quote
    property var part: ({})
    property bool expanded: false

    spacing: 12

    onExpandedChanged: {
        if (expanded && !nested.item)
            nested.setSource("Parts.qml", { model: quote.part.parts || [] })
    }

    Rectangle {
        width: Math.min(badgeRow.implicitWidth + 26, quote.width)
        height: 28
        radius: Theme.radius
        color: Qt.alpha(quote.expanded ? Theme.accent : Theme.foreground,
                        quote.expanded ? 0.16 : (hover.hovered ? 0.12 : 0.06))
        border.width: 1
        border.color: quote.expanded ? Qt.alpha(Theme.accent, 0.7) : Qt.alpha(Theme.line, 0.35)
        Behavior on color { ColorAnimation { duration: 120 } }

        Row {
            id: badgeRow
            anchors.left: parent.left
            anchors.leftMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8

            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: "quote"
                size: 13
                color: quote.expanded ? Theme.accent : Theme.muted
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                width: Math.min(implicitWidth, quote.width - 60)
                text: quote.part.attribution || Strings.quote
                color: quote.expanded ? Theme.foreground : Theme.muted
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize
                elide: Text.ElideRight
            }
        }

        HoverHandler {
            id: hover
            cursorShape: Qt.PointingHandCursor
        }

        TapHandler {
            onTapped: quote.expanded = !quote.expanded
        }
    }

    Item {
        width: quote.width
        height: nested.height
        visible: quote.expanded

        Rectangle {
            width: 2
            height: parent.height
            radius: 1
            color: Qt.alpha(Theme.accent, 0.5)
        }

        Loader {
            id: nested
            x: 18
            width: parent.width - 18
            opacity: 0.9
        }
    }
}
