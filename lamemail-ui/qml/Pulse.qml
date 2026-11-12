import QtQuick

SequentialAnimation {
    loops: Animation.Infinite
    alwaysRunToEnd: true

    NumberAnimation { from: 1; to: 0.3; duration: 700; easing.type: Easing.InOutSine }
    NumberAnimation { from: 0.3; to: 1; duration: 700; easing.type: Easing.InOutSine }
}
