import QtQuick
import Lamemail

TextEdit {
    id: paragraph
    property var part: ({})
    readonly property bool signature: part.kind === "signature"
    readonly property bool shortened: (part.shortened ?? []).includes(hoveredLink)

    text: "<style>a { color: " + Theme.accent + "; text-decoration: none }</style>" + (part.html || "")
    readOnly: true
    selectByMouse: true
    wrapMode: TextEdit.Wrap
    textFormat: TextEdit.RichText
    color: signature ? Theme.muted : Theme.foreground
    selectionColor: Qt.alpha(Theme.accent, 0.35)
    selectedTextColor: Theme.foregroundBright
    font.family: Theme.fontFamily
    font.pixelSize: signature ? Theme.fontSize : Theme.fontSize + 2
    onLinkActivated: (link) => Qt.openUrlExternally(link)

    HoverHandler {
        id: hover
        cursorShape: paragraph.hoveredLink ? Qt.PointingHandCursor : Qt.IBeamCursor
    }

    Tip {
        width: Math.min(implicitWidth, 520)
        wrapMode: Text.WrapAnywhere
        visible: paragraph.shortened
        text: paragraph.hoveredLink
        onAboutToShow: {
            const pointer = hover.point.position
            const line = paragraph.positionToRectangle(paragraph.positionAt(pointer.x, pointer.y))
            x = pointer.x
            y = line.y + line.height + 4
        }
    }

    PointHandler {
        acceptedButtons: Qt.LeftButton
        onActiveChanged: active ? copyDelay.stop() : copyDelay.restart()
    }

    Timer {
        id: copyDelay
        interval: Application.styleHints.mouseDoubleClickInterval
        onTriggered: {
            if (paragraph.selectedText.length === 0)
                return
            paragraph.copy()
            const end = paragraph.positionToRectangle(paragraph.selectionEnd)
            copyHint.show(Math.min(end.x + 8, paragraph.width - copyHint.width), end.y + end.height + 4)
        }
    }

    CopyHint {
        id: copyHint
        z: 1
    }
}
