import QtQuick
import Lamemail

MouseArea {
    id: area
    required property Flickable flickable
    property var step: null
    property real target: 0
    property real notches: 0

    anchors.fill: flickable
    acceptedButtons: Qt.NoButton
    cursorShape: undefined
    onWheel: (wheel) => {
        if (wheel.phase !== Qt.NoScrollPhase) {
            wheel.accepted = false
            return
        }
        const view = area.flickable
        const from = view.flicking ? area.target : view.contentY
        const turned = -wheel.angleDelta.y / 120
        let wanted = from + turned * Preferences.wheelPixels
        if (area.step) {
            area.notches += turned
            const whole = Math.trunc(area.notches)
            area.notches -= whole
            if (whole === 0)
                return
            wanted = area.step(from, whole)
        }
        const bottom = Math.max(view.originY, view.originY + view.contentHeight - view.height)
        area.target = Math.max(view.originY, Math.min(bottom, wanted))
        view.flickTo(Qt.point(view.contentX, area.target))
    }
}
