import QtQuick
import QtQuick.Effects
import Lamemail

Item {
    anchors.fill: parent

    Rectangle {
        id: shape
        anchors.fill: parent
        radius: Theme.radius
        color: Theme.surface
        visible: false
    }

    MultiEffect {
        source: shape
        anchors.fill: shape
        autoPaddingEnabled: true
        shadowEnabled: true
        shadowColor: "black"
        shadowOpacity: 0.35
        shadowBlur: 0.7
        shadowVerticalOffset: 4
    }
}
