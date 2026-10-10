import QtQuick

Item {
    property bool active: false

    opacity: active ? 1 : 0
    visible: opacity > 0
    enabled: active
    Behavior on opacity { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
}
