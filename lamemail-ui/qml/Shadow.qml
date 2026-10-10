import QtQuick
import Lamemail

Loader {
    anchors.fill: parent
    z: -1
    active: Preferences.shadows && Preferences.shadowsAvailable && GraphicsInfo.api !== GraphicsInfo.Software
    source: "ShadowEffect.qml"
}
