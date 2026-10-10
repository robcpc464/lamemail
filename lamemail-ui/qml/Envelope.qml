pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import Lamemail

Flow {
    id: envelope
    property string account
    property color tint: Theme.accent
    property bool canSwitchAccount: false
    property string defaultFrom
    property string from
    readonly property string domain: defaultFrom.includes("@") ? defaultFrom.slice(defaultFrom.lastIndexOf("@")) : ""
    property var people: []
    property alias typed: input.text
    readonly property var kinds: ["to", "cc", "bcc"]
    readonly property int line: 28
    signal accountSwitched()
    signal fromEdited()
    signal done()
    signal sendRequested()

    Contacts {
        id: known
        query: input.text.trim()
        onModelReset: list.currentIndex = 0
    }

    function editable(address) {
        const local = address.slice(0, address.lastIndexOf("@"))
        return envelope.domain.length > 0 && local + envelope.domain === address ? local : address
    }

    function showFrom() {
        const shown = envelope.editable(envelope.from)
        if (fromField.text !== shown)
            fromField.text = shown
    }

    function isSend(event) {
        return (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
                && (event.modifiers & Qt.ControlModifier) !== 0
    }

    function has(address) {
        const wanted = address.toLowerCase()
        return envelope.people.some(person => person.address.toLowerCase() === wanted)
    }

    function include(person, kind) {
        if (!envelope.has(person.address))
            envelope.people = envelope.people.concat([{ name: person.name, address: person.address, kind: kind }])
    }

    function exclude(address) {
        const wanted = address.toLowerCase()
        envelope.people = envelope.people.filter(person => person.address.toLowerCase() !== wanted)
    }

    function commit() {
        const text = input.text.trim()
        const named = text.match(/^(.*)<([^<>]+)>$/)
        if (!text.includes("@") || (text.includes("<") && !named))
            return false
        envelope.include({
            name: named ? named[1].trim().replace(/^"|"$/g, "") : "",
            address: named ? named[2].trim() : text
        }, "to")
        input.text = ""
        return true
    }

    function removeAt(index) {
        const people = envelope.people.slice()
        people.splice(index, 1)
        envelope.people = people
    }

    function cycle(index) {
        const people = envelope.people.slice()
        const person = people[index]
        const next = envelope.kinds[(envelope.kinds.indexOf(person.kind) + 1) % envelope.kinds.length]
        people[index] = { name: person.name, address: person.address, kind: next }
        envelope.people = people
    }

    function pick(contact, cc) {
        envelope.include(contact, cc ? "cc" : "to")
        input.text = ""
    }

    function focusInput() {
        input.forceActiveFocus()
    }

    onFromChanged: showFrom()
    onDomainChanged: showFrom()

    spacing: 6

    Item {
        visible: envelope.account.length > 0
        width: tag.width
        height: envelope.line

        Tag {
            id: tag
            anchors.verticalCenter: parent.verticalCenter
            text: envelope.account
            tint: envelope.tint
            opacity: envelope.canSwitchAccount && tagArea.containsMouse ? 0.75 : 1
            Behavior on opacity { NumberAnimation { duration: 120 } }
        }

        MouseArea {
            id: tagArea
            anchors.fill: parent
            anchors.margins: -4
            enabled: envelope.canSwitchAccount
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: envelope.accountSwitched()
        }
    }

    Rectangle {
        visible: envelope.account.length > 0
        width: senderRow.implicitWidth + 16
        height: envelope.line
        radius: Theme.radius
        color: "transparent"
        border.width: 1
        border.color: fromField.activeFocus ? Qt.alpha(Theme.accent, 0.7) : Qt.alpha(Theme.line, senderArea.containsMouse ? 0.7 : 0.35)
        Behavior on border.color { ColorAnimation { duration: 120 } }

        MouseArea {
            id: senderArea
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.IBeamCursor
            onClicked: {
                fromField.forceActiveFocus()
                fromField.cursorPosition = fromField.length
            }
        }

        Row {
            id: senderRow
            anchors.left: parent.left
            anchors.leftMargin: 8
            anchors.verticalCenter: parent.verticalCenter

            TextField {
                id: fromField
                height: envelope.line
                leftPadding: 0
                rightPadding: 0
                topPadding: 0
                bottomPadding: 0
                verticalAlignment: TextInput.AlignVCenter
                validator: RegularExpressionValidator {
                    regularExpression: envelope.domain.length > 0 ? /[^@\s<>,;"]*/ : /[^\s<>,;"]*/
                }
                placeholderText: envelope.domain.length > 0 ? "" : Strings.senderPlaceholder
                placeholderTextColor: Qt.alpha(Theme.muted, 0.8)
                color: Theme.foreground
                selectionColor: Qt.alpha(Theme.accent, 0.35)
                selectedTextColor: Theme.foregroundBright
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize
                background: null
                ContextMenu.menu: TextMenu {
                    editor: fromField
                }
                onTextEdited: {
                    envelope.from = fromField.text.length > 0 ? fromField.text + envelope.domain : ""
                    envelope.fromEdited()
                }
                onActiveFocusChanged: if (!activeFocus && text.length === 0) envelope.from = envelope.defaultFrom
                Keys.onEscapePressed: (event) => {
                    event.accepted = envelope.from !== envelope.defaultFrom
                    if (event.accepted)
                        envelope.from = envelope.defaultFrom
                }
                Keys.onPressed: (event) => {
                    if (envelope.isSend(event))
                        envelope.sendRequested()
                    else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
                        envelope.focusInput()
                    else
                        return
                    event.accepted = true
                }
            }

            Text {
                height: envelope.line
                verticalAlignment: Text.AlignVCenter
                text: envelope.domain
                color: fromField.activeFocus ? Theme.muted : Theme.foreground
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize
                Behavior on color { ColorAnimation { duration: 120 } }
            }
        }
    }

    Text {
        height: envelope.line
        leftPadding: 2
        rightPadding: 4
        verticalAlignment: Text.AlignVCenter
        text: "\u2192"
        color: Theme.muted
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize + 1
    }

    Repeater {
        model: envelope.people

        Rectangle {
            id: chip
            required property var modelData
            required property int index
            readonly property string label: ({ cc: Strings.ccKind, bcc: Strings.bccKind })[modelData.kind] ?? ""
            readonly property color tint: ({ cc: Theme.accent, bcc: Theme.yellow })[modelData.kind] ?? Theme.line
            width: chipRow.implicitWidth + 16
            height: envelope.line
            radius: Theme.radius
            color: "transparent"
            border.width: 1
            border.color: Qt.alpha(chip.tint, chipArea.containsMouse ? 0.7 : 0.35)
            Behavior on border.color { ColorAnimation { duration: 120 } }

            MouseArea {
                id: chipArea
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: envelope.cycle(chip.index)
            }

            Row {
                id: chipRow
                anchors.centerIn: parent
                spacing: 6

                Tag {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: chip.label.length > 0
                    text: chip.label
                    tint: chip.tint
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: chip.modelData.name.length > 0 ? chip.modelData.name : chip.modelData.address
                    color: Theme.foreground
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontSize - 1
                }

                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: "close"
                    size: 12
                    color: removeArea.containsMouse ? Theme.foregroundBright : Theme.muted

                    MouseArea {
                        id: removeArea
                        anchors.fill: parent
                        anchors.margins: -6
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: envelope.removeAt(chip.index)
                    }
                }
            }
        }
    }

    TextField {
        id: input
        width: Math.max(180, envelope.width / 3)
        height: envelope.line
        leftPadding: 4
        rightPadding: 4
        placeholderText: Strings.addressPlaceholder
        placeholderTextColor: Qt.alpha(Theme.muted, 0.8)
        color: Theme.foreground
        selectionColor: Qt.alpha(Theme.accent, 0.35)
        selectedTextColor: Theme.foregroundBright
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize
        background: null
        onActiveFocusChanged: {
            if (activeFocus)
                known.reload()
            else
                envelope.commit()
        }
        ContextMenu.menu: TextMenu {
            editor: input
        }
        Keys.onEscapePressed: (event) => {
            event.accepted = suggestions.visible
            if (event.accepted)
                input.text = ""
        }
        Keys.onPressed: (event) => {
            const separator = event.key === Qt.Key_Comma || event.key === Qt.Key_Semicolon
            const enter = event.key === Qt.Key_Return || event.key === Qt.Key_Enter
            const suggesting = suggestions.visible
            if (envelope.isSend(event)) {
                envelope.commit()
                envelope.sendRequested()
                event.accepted = true
            } else if (suggesting && event.key === Qt.Key_Down) {
                list.incrementCurrentIndex()
                event.accepted = true
            } else if (suggesting && event.key === Qt.Key_Up) {
                list.decrementCurrentIndex()
                event.accepted = true
            } else if (suggesting && (enter || event.key === Qt.Key_Tab)) {
                const row = list.currentItem as ContactRow
                if (row)
                    envelope.pick(row, false)
                event.accepted = true
            } else if (enter && input.text.trim().length === 0) {
                envelope.done()
                event.accepted = true
            } else if (separator || enter) {
                event.accepted = envelope.commit()
            } else if (event.key === Qt.Key_Backspace && input.text.length === 0 && envelope.people.length > 0) {
                envelope.removeAt(envelope.people.length - 1)
                event.accepted = true
            }
        }

        Popup {
            id: suggestions
            readonly property int gap: 6
            readonly property real tallest: 5 * 52 + topPadding + bottomPadding
            property bool above: false
            visible: input.activeFocus && input.text.trim().length > 0 && list.count > 0
            y: above ? -height - gap : input.height + gap
            width: 420
            height: Math.min(list.contentHeight + topPadding + bottomPadding, tallest)
            margins: 8
            padding: 4
            closePolicy: Popup.NoAutoClose
            onAboutToShow: above = input.mapToItem(null, 0, input.height).y + gap + tallest + margins > Overlay.overlay.height
            background: Rectangle {
                radius: Theme.radius
                color: Theme.background
                border.width: 1
                border.color: Qt.alpha(Theme.line, 0.35)
            }

            contentItem: Item {
                ListView {
                    id: list
                    anchors.fill: parent
                    model: known
                    clip: true
                    currentIndex: 0
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: SlimScrollBar {}

                    delegate: ContactRow {
                        id: suggestion
                        chosen: envelope.has(suggestion.address)
                        onPicked: (cc) => envelope.pick(suggestion, cc)
                    }
                }

                WheelArea {
                    flickable: list
                }
            }
        }
    }
}
