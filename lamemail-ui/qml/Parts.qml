pragma ComponentBehavior: Bound

import QtQuick

Column {
    id: parts
    property var model: []

    spacing: 14

    Repeater {
        model: parts.model

        Column {
            id: piece
            required property var modelData

            width: parts.width

            Paragraph {
                width: piece.width
                visible: piece.modelData.kind !== "quote"
                part: piece.modelData
            }

            Quote {
                width: piece.width
                visible: piece.modelData.kind === "quote"
                part: piece.modelData
            }
        }
    }
}
