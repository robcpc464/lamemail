pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import Lamemail

View {
    id: page
    readonly property color tint: Theme[mail.color] ?? Theme.accent
    readonly property string account: mail.account
    property var drafts: ({})
    property bool filled: false
    property bool forwarding: false
    property var replyPeople: null
    property string opened
    property bool seen: true
    property bool noting: false
    property bool stale: false
    property string inlineShownFor
    readonly property var ownAttachments: mail.attachments.filter(item => !item.inline)
    readonly property var inlineAttachments: mail.attachments.filter(item => item.inline)
    readonly property bool inlineShown: page.inlineShownFor === mail.path
    readonly property bool all: mail.answerCc.length > 0 && mail.answerCc.every(person => composer.has(person.address))
    signal closeRequested()
    signal composeRequested()
    signal settingsRequested()
    signal sendRequested(var job, string recipients)
    signal draftRequested(var job)
    signal discardRequested(var job)
    signal flagRequested(string path, bool flagged)
    signal seenRequested(string path, bool seen)
    signal moveRequested(string path, string target)
    signal deleteRequested(string path)
    signal failed(string error)
    signal attachmentsDownloaded(int count, string folder)

    Mail {
        id: mail
        onChanged: {
            if (page.filled || !page.loaded())
                return
            page.filled = true
            composer.restore(page.answer(composer.text))
        }
        onAttachmentReady: (url) => opener.open(url)
        onAttachmentFailed: (error) => page.failed(error)
        onAttachmentsDownloaded: (count, folder) => page.attachmentsDownloaded(count, folder)
    }

    Opener {
        id: opener
        onFailed: (message) => page.failed(message)
    }

    function loaded() {
        return mail.answerSubject.length > 0
    }

    function answer(text) {
        return {
            from: mail.answerFrom,
            people: mail.answerTo.map(person => ({ name: person.name, address: person.address, kind: "to" })),
            subject: page.subjectFor(page.forwarding),
            text: text
        }
    }

    function subjectFor(forwarding) {
        return forwarding ? mail.forwardSubject : mail.answerSubject
    }

    function draft() {
        return Object.assign(composer.draft(), { filled: page.filled, forward: page.forwarding, replyPeople: page.replyPeople,
                                                 opened: page.opened })
    }

    function keepDraft() {
        if (mail.path.length > 0)
            page.drafts[mail.path] = page.draft()
    }

    function toggleForward() {
        const before = page.subjectFor(page.forwarding)
        page.forwarding = !page.forwarding
        composer.subject = composer.subject === before ? page.subjectFor(page.forwarding)
                                                       : page.retitled(composer.subject)
        const own = composer.attachments.filter(item => item.source !== mail.path)
        const inDraft = (item) => own.some(other => other.path === undefined && other.source === undefined
                                                    && other.name === item.name && other.type === item.type
                                                    && other.size === item.size)
        const forwarded = page.ownAttachments.filter(item => !inDraft(item)).map(item => Object.assign({ source: mail.path }, item))
        composer.attachments = page.forwarding ? forwarded.concat(own) : own
        if (page.forwarding) {
            page.replyPeople = composer.people
            const answered = mail.answerTo.map(person => person.address.toLowerCase())
            composer.people = composer.people.filter(person => !answered.includes(person.address.toLowerCase()))
        } else if (page.replyPeople) {
            composer.people = page.replyPeople
            page.replyPeople = null
        }
        page.keepDraft()
        composer.focusText()
    }

    function retitled(subject) {
        const rest = subject.replace(/^(\s*(re|aw|fwd|fw|wg)\s*:)+\s*/i, "")
        return ((page.forwarding ? "Fwd: " : "Re: ") + rest).trim()
    }

    function show(path) {
        const draft = page.drafts[path]
        page.stale = false
        mail.open(path)
        page.seen = true
        page.forwarding = draft?.forward === true
        page.replyPeople = draft?.replyPeople ?? null
        page.opened = draft?.opened ?? ""
        page.filled = draft?.filled === true || page.loaded()
        composer.restore(draft?.filled ? draft : page.answer(draft?.text ?? ""))
        body.contentY = 0
    }

    function toggleAll() {
        if (page.all) {
            const others = mail.answerCc.map(person => person.address.toLowerCase())
            composer.people = composer.people.filter(person => person.kind !== "cc" || !others.includes(person.address.toLowerCase()))
        } else {
            mail.answerCc.forEach(person => composer.add(person, "cc"))
        }
        composer.focusText()
    }

    function job() {
        return Object.assign(composer.job(), { path: mail.path, forward: page.forwarding, draft: page.draft(), opened: page.opened })
    }

    function send() {
        if (composer.canSend)
            page.sendRequested(page.job(), composer.names())
    }

    function saveDraft() {
        if (composer.canSave)
            page.draftRequested(page.job())
    }

    function discard() {
        if (composer.canDiscard)
            page.discardRequested(page.job())
    }

    function drafted(path, before, draftId) {
        if (mail.path === path && composer.draftId === before) {
            composer.draftId = draftId
            composer.attachments = composer.kept(composer.attachments, draftId)
            page.keepDraft()
        } else if (mail.path !== path && page.drafts[path]?.draftId === before) {
            page.drafts[path].draftId = draftId
            page.drafts[path].attachments = composer.kept(page.drafts[path].attachments ?? [], draftId)
        }
    }

    function toggleFlag() {
        mail.flagged = !mail.flagged
        star.pop()
        page.flagRequested(mail.path, mail.flagged)
    }

    function toggleSeen() {
        page.seen = !page.seen
        seenButton.pop()
        page.seenRequested(mail.path, page.seen)
    }

    function forget(path, text) {
        if (page.drafts[path]?.text !== text)
            return
        delete page.drafts[path]
        if (mail.path !== path)
            return
        page.forwarding = false
        page.replyPeople = null
        page.opened = ""
        composer.restore(page.answer(""))
    }

    function continueDraft(path, draft) {
        page.drafts[path] = Object.assign({}, draft, { filled: true })
    }

    function restore(path, draft) {
        if (page.drafts[path])
            return
        page.drafts[path] = Object.assign({}, draft, { filled: true })
        if (mail.path !== path)
            return
        page.forwarding = draft.forward === true
        page.replyPeople = draft.replyPeople ?? null
        page.opened = draft.opened ?? ""
        composer.restore(draft)
    }

    function refresh() {
        if (active)
            mail.reload()
        else
            page.stale = true
    }

    onActiveChanged: {
        if (!active)
            return
        if (page.stale)
            mail.reload()
        page.stale = false
        composer.focusText()
    }

    Keys.onEscapePressed: page.closeRequested()

    Backdrop {}

    PageHeader {
        id: header
        title: mail.subject.length > 0 ? mail.subject : "—"
        tag: mail.account
        tint: page.tint
        onBackRequested: page.closeRequested()

        IconButton {
            name: "compose"
            tip: Strings.composeTip
            onClicked: page.composeRequested()
        }

        IconButton {
            name: "settings"
            tip: Strings.settingsTip
            dot: Preferences.checkUpdates && Updates.newer
            onClicked: page.settingsRequested()
        }
    }

    Split {
        anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        category: "detail"

        Flickable {
            id: body
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: actionBar.top
            contentHeight: column.height + 56
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            flickDeceleration: 6000
            maximumFlickVelocity: 4000
            ScrollBar.vertical: SlimScrollBar {}

            Column {
                id: column
                x: 28
                y: 28
                width: body.width - 56
                spacing: 18

                Item {
                    width: column.width
                    height: senderText.height

                    Text {
                        id: senderText
                        anchors.left: parent.left
                        anchors.right: timeText.left
                        anchors.rightMargin: 16
                        text: mail.sender
                        color: Theme.foregroundBright
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontSize + 2
                        font.bold: true
                        elide: Text.ElideRight
                    }

                    Text {
                        id: timeText
                        anchors.right: parent.right
                        anchors.baseline: senderText.baseline
                        text: mail.time
                        color: Theme.muted
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontSize
                    }
                }

                Item {
                    width: column.width
                    height: Math.max(recipientsText.height, replyAllChip.visible ? replyAllChip.height : 0)
                    visible: mail.recipients.length > 0

                    Text {
                        id: recipientsText
                        anchors.left: parent.left
                        anchors.right: replyAllChip.visible ? replyAllChip.left : parent.right
                        anchors.rightMargin: replyAllChip.visible ? 12 : 0
                        anchors.verticalCenter: replyAllChip.visible && lineCount === 1 ? parent.verticalCenter : undefined
                        text: Strings.to + "  " + mail.recipients
                        color: Theme.muted
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontSize
                        wrapMode: Text.Wrap
                    }

                    Pill {
                        id: replyAllChip
                        anchors.right: parent.right
                        visible: mail.answerCc.length > 0
                        icon: "replyAll"
                        on: page.all
                        tip: Strings.replyAll
                        onClicked: page.toggleAll()
                    }
                }

                Flow {
                    width: column.width
                    spacing: 8
                    visible: mail.attachments.length > 0

                    Component {
                        id: fileChip

                        AttachmentChip {
                            required property var modelData
                            name: modelData.name
                            size: modelData.size
                            busy: mail.opening.includes(modelData.index)
                            onClicked: mail.openAttachment(modelData.index)
                        }
                    }

                    Repeater {
                        model: page.ownAttachments
                        delegate: fileChip
                    }

                    AttachmentChip {
                        visible: page.ownAttachments.length > 0
                        icon: "download"
                        tip: Strings.downloadAll
                        busy: mail.downloading
                        onClicked: mail.downloadAttachments(Preferences.downloads())
                    }

                    AttachmentChip {
                        visible: page.inlineAttachments.length > 0
                        icon: "image"
                        name: Strings.inlineImages.replace("{}", page.inlineAttachments.length)
                        tip: page.inlineShown ? Strings.hideInline : Strings.showInline
                        onClicked: page.inlineShownFor = page.inlineShown ? "" : mail.path
                    }

                    Repeater {
                        model: page.inlineShown ? page.inlineAttachments : []
                        delegate: fileChip
                    }
                }

                Text {
                    width: column.width
                    visible: mail.error.length > 0
                    text: mail.error
                    color: Theme.red
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontSize
                    wrapMode: Text.Wrap
                }

                Parts {
                    width: column.width
                    model: mail.parts
                }

                Text {
                    visible: mail.parts.length === 0 && !mail.loading && mail.error.length === 0
                    text: Strings.noText
                    color: Theme.muted
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontSize + 1
                }
            }
        }

        WheelArea {
            flickable: body
        }

        Item {
            id: actionBar
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 56

            Rule {
                anchors.top: parent.top
                anchors.bottom: undefined
            }

            Row {
                anchors.left: parent.left
                anchors.leftMargin: 20
                anchors.verticalCenter: parent.verticalCenter
                spacing: 4

                IconButton {
                    id: star
                    name: "star"
                    tip: mail.flagged ? Strings.unstarTip : Strings.starTip
                    filled: mail.flagged
                    iconColor: mail.flagged ? Theme.yellow : Theme.muted
                    onClicked: page.toggleFlag()
                }

                IconButton {
                    id: seenButton
                    name: page.seen ? "mail" : "unread"
                    tip: page.seen ? Strings.unreadTip : Strings.readTip
                    iconColor: page.seen ? Theme.muted : Theme.accent
                    onClicked: page.toggleSeen()
                }

                Item {
                    width: 9
                    height: 36

                    Rectangle {
                        anchors.centerIn: parent
                        width: 1
                        height: 18
                        color: Theme.line
                        opacity: 0.5
                    }
                }

                Repeater {
                    model: Styles.moves(mail.kind)

                    IconButton {
                        required property string modelData
                        name: modelData
                        danger: modelData === "trash"
                        tip: ({ archive: Strings.archiveTip, trash: Strings.trashTip, inbox: Strings.inboxTip })[modelData]
                        onClicked: page.moveRequested(mail.path, modelData)
                    }
                }

                IconButton {
                    visible: mail.kind === "trash"
                    name: "trash"
                    danger: true
                    tip: Strings.deleteTip
                    onClicked: page.deleteRequested(mail.path)
                }
            }
        }

        second: Composer {
            id: composer
            anchors.fill: parent
            account: mail.account
            tint: page.tint
            defaultFrom: mail.answerFrom
            placeholder: Strings.replyPlaceholder
            allowed: (mail.error.length === 0 || composer.draftId.length > 0) && !page.noting
            mode: page.forwarding ? "forward" : "reply"
            quote: mail.quote
            signature: mail.signature
            replyTo: mail.path
            onEdited: page.keepDraft()
            onSendRequested: page.send()
            onSaveRequested: page.saveDraft()
            onDiscardRequested: page.discard()
            onModeToggled: page.toggleForward()
            onSuggestionFailed: (error) => page.failed(error)
            onWarning: (message) => page.failed(message)
        }
    }
}
