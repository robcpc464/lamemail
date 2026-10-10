pragma ComponentBehavior: Bound

import QtQuick

Row {
    id: choice
    property var options: []
    property var labels: []
    property var value
    signal chosen(var value)

    spacing: 6

    Repeater {
        model: choice.options

        Pill {
            id: option
            required property var modelData
            required property int index
            text: choice.labels[index] ?? modelData
            on: modelData === choice.value
            onClicked: choice.chosen(option.modelData)
        }
    }
}
