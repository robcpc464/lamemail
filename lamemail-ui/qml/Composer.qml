pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import Lamemail

Item {
    id: composer
    property alias account: envelope.account
    property alias tint: envelope.tint
    property alias canSwitchAccount: envelope.canSwitchAccount
    property alias defaultFrom: envelope.defaultFrom
    property alias from: envelope.from
    property alias people: envelope.people
    property alias subject: subjectField.text
    property alias text: field.text
    property alias placeholder: field.placeholderText
    property bool allowed: true
    property string mode
    property var attachments: []
    property string quote
    property string signature
    property string draftId
    property string replyTo
    property var place: ({ from: 0, to: 0 })
    property var exchange: ({ instruction: "", answers: [], asked: 0 })
    property string question
    readonly property int questionLimit: 3
    property date now: new Date()
    property string warned
    property var report: null
    readonly property var commands: {
        const commands = []
        if (quote.length > 0)
            commands.push({ name: Strings.quoteCommand, label: Strings.quoteCommandHint, icon: "quote", text: quote, fold: true,
                            enabled: true })
        commands.push({ name: Strings.aiCommand, icon: "sparkles", prompt: true, enabled: Ai.ready,
                        label: Ai.ready ? Strings.aiCommandHint : Strings.aiCommandOff })
        commands.push({ name: Strings.signatureCommand, icon: "signature", text: "-- \n" + signature, line: true,
                        enabled: signature.length > 0,
                        label: signature.length > 0 ? Strings.signatureCommandHint : Strings.noSignature })
        commands.push({ name: Strings.attachCommand, label: Strings.attachTip, icon: "attachment", enabled: true,
                        run: () => fileDialog.open() })
        commands.push({ name: Strings.dateCommand, label: Strings.dateCommandHint, icon: "calendar", enabled: true,
                        values: Strings.dates(composer.now) })
        return commands
    }
    property int commandAt: -1
    property int dismissedAt: -1
    property string commandQuery
    property rect commandCursor
    readonly property var offered: {
        if (commandAt < 0)
            return []
        const space = commandQuery.indexOf(" ")
        const name = space < 0 ? commandQuery : commandQuery.slice(0, space)
        const typed = space < 0 ? "" : commandQuery.slice(space + 1)
        const matching = commands.filter(command => space < 0 ? command.name.startsWith(name)
                                                               : command.name === name
                                                                 && (command.prompt || /^\w*$/.test(typed) && command.values !== undefined))
        if (matching.length !== 1 || matching[0].values === undefined)
            return matching
        const command = matching[0]
        return command.values.filter(value => value.name.startsWith(typed))
            .map(value => ({ name: command.name + " " + value.name, label: value.text, icon: command.icon, text: value.text,
                             enabled: true }))
    }
    readonly property bool addressed: allowed && account.length > 0 && /^[^@\s]+@[^@\s]+$/.test(from)
    readonly property bool canSend: addressed && !suggester.running && people.some(person => person.kind === "to")
                                    && text.trim().length > 0
    readonly property bool empty: text.trim().length === 0 && attachments.length === 0
    readonly property bool canSave: addressed && !suggester.running && !empty
    readonly property bool canDiscard: allowed && draftId.length > 0
    signal accountSwitched()
    signal fromEdited()
    signal edited()
    signal sendRequested()
    signal warning(string message)
    signal saveRequested()
    signal discardRequested()
    signal modeToggled()
    signal suggestionFailed(string error)

    function focusText() {
        field.forceActiveFocus()
    }

    function add(person, kind) {
        envelope.include(person, kind)
    }

    function remove(address) {
        envelope.exclude(address)
    }

    function has(address) {
        return envelope.has(address)
    }

    function addresses(kind) {
        return composer.people.filter(person => person.kind === kind)
            .map(person => person.name.length > 0 ? person.name + " <" + person.address + ">" : person.address)
    }

    function job() {
        return {
            account: composer.account,
            from: composer.from !== composer.defaultFrom ? composer.from : "",
            to: composer.addresses("to"),
            cc: composer.addresses("cc"),
            bcc: composer.addresses("bcc"),
            subject: composer.subject,
            text: composer.text,
            files: composer.attachments.filter(item => item.path).map(item => item.path),
            references: composer.attachments.filter(item => item.path === undefined)
                .map(item => item.source ? item.source + ":" + item.index : String(item.index)),
            draftId: composer.draftId,
            draft: composer.draft()
        }
    }

    function attach(urls) {
        const known = composer.attachments.map(item => item.path)
        const added = LocalFiles.describe(urls).filter(file => !known.includes(file.path))
        if (added.length > 0)
            composer.attachments = composer.attachments.concat(added)
    }

    function kept(attachments, draftId) {
        const drafted = item => item.path === undefined && item.source === undefined
        if (draftId.length === 0)
            return attachments.filter(item => !drafted(item))
        const indices = attachments.filter(drafted).map(item => item.index).sort((a, b) => a - b)
        return attachments.map(item => drafted(item) ? Object.assign({}, item, { index: indices.indexOf(item.index) }) : item)
    }

    function detach(index) {
        composer.attachments = composer.attachments.filter((item, at) => at !== index)
        composer.focusText()
    }

    function findCommand() {
        const before = field.text.slice(0, field.cursorPosition)
        const match = before.match(/(^|\s)\/(\w*(?: [^\n]*)?)$/)
        if (match)
            composer.now = new Date()
        else
            composer.dismissedAt = -1
        const at = match ? before.length - match[2].length - 1 : -1
        composer.commandAt = at === composer.dismissedAt ? -1 : at
        composer.commandQuery = match ? match[2] : ""
        const cursor = field.cursorRectangle
        const top = field.mapToItem(composer, cursor.x, cursor.y)
        composer.commandCursor = Qt.rect(top.x, top.y, cursor.width, cursor.height)
    }

    function runCommand(command) {
        const at = composer.commandAt
        const space = composer.commandQuery.indexOf(" ")
        field.forceActiveFocus()
        if (command.prompt && space >= 0) {
            composer.ask(composer.commandQuery.slice(space + 1))
            return
        }
        if (command.values !== undefined || command.prompt) {
            field.remove(at + 1, field.cursorPosition)
            field.insert(at + 1, command.name + " ")
            field.cursorPosition = at + command.name.length + 2
            return
        }
        composer.commandAt = -1
        field.remove(at, field.cursorPosition)
        if (command.text !== undefined)
            composer.insertCommand(at, command)
        if (command.run)
            command.run()
    }

    function ask(instruction) {
        composer.place = { from: composer.commandAt, to: field.cursorPosition }
        composer.exchange = { instruction: instruction.trim(), answers: [], asked: 0 }
        composer.question = ""
        composer.dismiss()
        composer.run(false)
    }

    function run(written) {
        const exchange = composer.exchange
        suggester.suggest({
            account: composer.account,
            reply: composer.replyTo,
            instruction: [exchange.instruction].concat(exchange.answers).join("\n"),
            to: composer.addresses("to"),
            cc: composer.addresses("cc"),
            subject: composer.subject,
            before: field.text.slice(0, composer.place.from),
            after: field.text.slice(composer.place.to),
            written: written || exchange.asked >= composer.questionLimit
        })
    }

    function takeQuestion(question) {
        composer.exchange = Object.assign({}, composer.exchange, { asked: composer.exchange.asked + 1 })
        composer.question = question
        aiPanel.focusAnswer()
    }

    function answer(text) {
        const answers = composer.exchange.answers.concat([composer.question + " " + text])
        composer.exchange = Object.assign({}, composer.exchange, { answers: answers })
        composer.question = ""
        composer.focusText()
        composer.run(false)
    }

    function skipQuestion() {
        composer.question = ""
        composer.focusText()
        composer.run(true)
    }

    function stopAsking() {
        suggester.cancel()
        composer.question = ""
        composer.focusText()
        composer.reopen()
    }

    function reopen() {
        composer.dismissedAt = -1
        composer.findCommand()
    }

    function takeSuggestion(text) {
        composer.question = ""
        folding.replace(text)
        field.cursorPosition = Math.min(composer.place.from, field.length)
        composer.focusText()
    }

    function insertCommand(at, command) {
        const text = command.line && at > 0 && field.text[at - 1] !== "\n" ? "\n" + command.text : command.text
        field.insert(at, text)
        field.cursorPosition = at + text.length
        if (command.fold)
            folding.fold(at)
    }

    function commandKey(event) {
        if (composer.offered.length === 0)
            return false
        const usable = composer.offered.map((command, index) => command.enabled ? index : -1).filter(index => index >= 0)
        const at = usable.indexOf(commandMenu.current)
        if (event.key === Qt.Key_Escape)
            composer.dismiss()
        else if (usable.length === 0)
            return false
        else if (event.key === Qt.Key_Up)
            commandMenu.current = usable[Math.max(0, at - 1)]
        else if (event.key === Qt.Key_Down)
            commandMenu.current = usable[Math.min(usable.length - 1, at + 1)]
        else if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Tab)
                 && !(event.modifiers & Qt.ControlModifier))
            composer.runCommand(composer.offered[commandMenu.current])
        else
            return false
        return true
    }

    function dismiss() {
        composer.dismissedAt = composer.commandAt
        composer.commandAt = -1
    }

    function names() {
        return composer.people.map(person => person.name.length > 0 ? person.name : person.address).join(", ")
    }

    function draft() {
        return { from: composer.from, people: composer.people, subject: composer.subject, text: composer.text,
                 attachments: composer.attachments, draftId: composer.draftId }
    }

    function restore(state) {
        suggester.cancel()
        composer.question = ""
        composer.report = null
        envelope.typed = ""
        composer.from = state.from
        composer.people = state.people
        composer.subject = state.subject
        composer.text = state.text
        composer.attachments = state.attachments ?? []
        composer.draftId = state.draftId ?? ""
        folding.foldAll()
    }

    function gap() {
        const written = field.text.split("\n").filter(line => !/^\s*>/.test(line)).join("\n")
        return written.match(/\[[^\[\]\n]{1,40}\]/)?.[0] ?? ""
    }

    function requestSend() {
        const gap = composer.gap()
        if (gap.length > 0 && composer.canSend && composer.warned !== field.text) {
            composer.warned = field.text
            composer.warning(Strings.gapLeft.replace("{}", gap))
            return
        }
        composer.sendRequested()
    }

    function sendOrNext(event, next) {
        if (event.key !== Qt.Key_Return && event.key !== Qt.Key_Enter)
            return
        if (event.modifiers & Qt.ControlModifier)
            composer.requestSend()
        else if (next)
            next()
        else
            return
        event.accepted = true
    }

    onOfferedChanged: commandMenu.current = offered.findIndex(command => command.enabled)
    onFromChanged: edited()
    onPeopleChanged: edited()
    onSubjectChanged: edited()
    onTextChanged: edited()
    onAttachmentsChanged: edited()

    Shortcut {
        sequence: "Ctrl+S"
        autoRepeat: false
        enabled: composer.enabled
        onActivated: composer.saveRequested()
    }

    Suggester {
        id: suggester
        onSuggested: (text) => composer.takeSuggestion(text)
        onAsked: (question) => composer.takeQuestion(question)
        onReported: (report) => composer.report = report
        onFailed: (error) => {
            composer.reopen()
            composer.suggestionFailed(error)
        }
    }

    FileDialog {
        id: fileDialog
        title: Strings.attachTip
        fileMode: FileDialog.OpenFiles
        onAccepted: {
            composer.attach(selectedFiles)
            composer.focusText()
        }
        onRejected: composer.focusText()
    }

    Envelope {
        id: envelope
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 16
        anchors.rightMargin: 16
        anchors.topMargin: 12
        onAccountSwitched: composer.accountSwitched()
        onFromEdited: composer.fromEdited()
        onDone: subjectField.forceActiveFocus()
        onSendRequested: composer.requestSend()
    }

    Rectangle {
        anchors.top: envelope.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: footer.top
        anchors.leftMargin: 16
        anchors.rightMargin: 16
        anchors.topMargin: 10
        radius: Theme.radius
        color: Qt.alpha(Theme.foreground, 0.04)
        border.width: 1
        border.color: drop.containsDrag || subjectField.activeFocus || field.activeFocus ? Qt.alpha(Theme.accent, 0.7) : Qt.alpha(Theme.line, 0.35)
        Behavior on border.color { ColorAnimation { duration: 150 } }

        Pill {
            id: modeChip
            anchors.left: parent.left
            anchors.leftMargin: 12
            anchors.verticalCenter: subjectField.verticalCenter
            visible: composer.mode.length > 0
            icon: composer.mode
            text: composer.mode === "forward" ? Strings.forwardMode : Strings.replyMode
            tip: Strings.modeTip
            onClicked: composer.modeToggled()
        }

        TextField {
            id: subjectField
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: 2
            anchors.leftMargin: modeChip.visible ? modeChip.x + modeChip.width + 2 : 2
            leftPadding: modeChip.visible ? 6 : 12
            rightPadding: 12
            topPadding: 10
            bottomPadding: 10
            placeholderText: Strings.subject
            placeholderTextColor: Qt.alpha(Theme.muted, 0.8)
            color: Theme.foregroundBright
            selectionColor: Qt.alpha(Theme.accent, 0.35)
            selectedTextColor: Theme.foregroundBright
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize + 2
            font.bold: true
            background: null
            Keys.onPressed: (event) => composer.sendOrNext(event, composer.focusText)
            ContextMenu.menu: TextMenu {
                editor: subjectField
            }
        }

        Rectangle {
            id: divider
            anchors.top: subjectField.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            height: 1
            color: Qt.alpha(Theme.line, 0.35)
        }

        ScrollView {
            anchors.top: divider.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: aiPanel.visible ? aiPanel.top : (chips.visible ? chips.top : parent.bottom)
            anchors.margins: 2

            TextArea {
                id: field
                leftPadding: 12
                rightPadding: 12
                topPadding: 10
                bottomPadding: 10
                placeholderTextColor: Qt.alpha(Theme.muted, 0.8)
                wrapMode: TextArea.Wrap
                color: Theme.foreground
                selectionColor: Qt.alpha(Theme.accent, 0.35)
                selectedTextColor: Theme.foregroundBright
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize + 2
                background: null
                readOnly: suggester.running || composer.question.length > 0
                Keys.onPressed: (event) => {
                    if (suggester.running && event.key === Qt.Key_Escape) {
                        composer.stopAsking()
                        event.accepted = true
                    } else if (composer.commandKey(event)) {
                        event.accepted = true
                    } else {
                        composer.sendOrNext(event, null)
                    }
                }
                onCursorPositionChanged: {
                    composer.findCommand()
                    folding.reveal(cursorPosition)
                }
                onTextChanged: composer.findCommand()
                ContextMenu.menu: TextMenu {
                    editor: field
                }

                Repeater {
                    model: folding.folds

                    Rectangle {
                        id: marker
                        required property var modelData
                        readonly property rect line: field.width > 0 && field.contentHeight > 0
                                                     ? field.positionToRectangle(modelData.end) : Qt.rect(0, 0, 0, 0)

                        x: Math.min(line.x + 10, field.width - width - field.rightPadding)
                        y: line.y + (line.height - height) / 2
                        width: markerRow.implicitWidth + 14
                        height: 20
                        radius: Theme.radius
                        color: Qt.alpha(Theme.foreground, markerHover.hovered ? 0.14 : 0.07)

                        Row {
                            id: markerRow
                            anchors.centerIn: parent
                            spacing: 4

                            Icon {
                                anchors.verticalCenter: parent.verticalCenter
                                name: marker.modelData.folded ? "chevronRight" : "chevronDown"
                                size: 12
                                color: Theme.muted
                            }

                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: marker.modelData.folded
                                text: Strings.foldedLines.replace("{}", marker.modelData.lines)
                                color: Theme.muted
                                font.family: Theme.fontFamily
                                font.pixelSize: Theme.fontSize - 2
                            }
                        }

                        HoverHandler {
                            id: markerHover
                            cursorShape: Qt.PointingHandCursor
                        }

                        Tip {
                            visible: markerHover.hovered
                            text: Strings.foldTip
                        }

                        TapHandler {
                            onTapped: {
                                field.cursorPosition = marker.modelData.end
                                folding.toggle(marker.modelData.position)
                            }
                        }
                    }
                }
            }
        }

        Folding {
            id: folding
            document: field.textDocument
        }

        AiPanel {
            id: aiPanel
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: chips.visible ? chips.top : parent.bottom
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            anchors.bottomMargin: chips.visible ? 8 : 12
            visible: running || asking || reporting
            running: suggester.running
            question: composer.question
            report: composer.report
            maximumHeight: parent.height * 0.4
            onAnswered: (text) => composer.answer(text)
            onSkipped: composer.skipQuestion()
            onCancelled: composer.stopAsking()
            onClosed: composer.report = null
        }

        Flow {
            id: chips
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: 12
            spacing: 8
            visible: composer.attachments.length > 0

            Repeater {
                model: composer.attachments

                AttachmentChip {
                    id: chip
                    required property int index
                    required property var modelData
                    icon: chip.hovered ? "close" : "attachment"
                    tip: Strings.remove
                    name: modelData.name
                    size: modelData.size
                    onClicked: composer.detach(chip.index)
                }
            }
        }
    }

    Item {
        id: footer
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.leftMargin: 16
        anchors.rightMargin: 16
        height: 56

        IconButton {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            name: "attachment"
            tip: Strings.attachTip
            onClicked: fileDialog.open()
        }

        Row {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: 4

            IconButton {
                visible: composer.draftId.length > 0
                opacity: composer.canDiscard ? 1 : 0.4
                name: "trash"
                danger: true
                tip: Strings.discardTip
                onClicked: composer.discardRequested()
            }

            IconButton {
                visible: !composer.empty
                opacity: composer.canSave ? 1 : 0.4
                name: "save"
                tip: Strings.saveDraftTip
                onClicked: composer.saveRequested()
            }

            ActionButton {
                icon: "outbox"
                tip: Strings.sendTip
                ready: composer.canSend
                onClicked: composer.requestSend()
            }
        }
    }

    CommandMenu {
        id: commandMenu
        z: 2
        visible: composer.offered.length > 0 && field.activeFocus
        commands: composer.offered
        x: Math.max(16, Math.min(composer.commandCursor.x, composer.width - width - 16))
        y: composer.commandCursor.y + composer.commandCursor.height + 4 + height < footer.y
           ? composer.commandCursor.y + composer.commandCursor.height + 4
           : composer.commandCursor.y - height - 4
        onChosen: (command) => composer.runCommand(command)
    }

    DropArea {
        id: drop
        anchors.fill: parent
        keys: ["text/uri-list"]
        onDropped: (event) => {
            composer.attach(event.urls)
            event.accept(Qt.CopyAction)
        }
    }
}
