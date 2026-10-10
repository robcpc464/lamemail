pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import Lamemail

View {
    id: page
    property string account: ""
    property bool accountChosen: false
    property bool openingDraft: false
    property bool noting: false
    signal closeRequested()
    signal sendRequested(var job, string recipients)
    signal draftRequested(var job)
    signal discardRequested(var job)
    signal failed(string error)
    signal draftOpened()
    signal replyContinued(string path, var draft)

    Contacts {
        id: contacts
        chosen: composer.people.map(person => person.address)
        onModelReset: Qt.callLater(page.toFirstContact)
    }

    Accounts {
        id: accounts
        onIdsChanged: page.follow()
    }

    Mail {
        id: draftMail
        onLoadingChanged: if (!loading) page.continueDraft()
    }

    function refresh() {
        if (active) contacts.reload()
    }

    function add(contact, cc) {
        composer.add(contact, cc ? "cc" : "to")
    }

    function pick(contact, cc) {
        if (composer.has(contact.address))
            composer.remove(contact.address)
        else
            page.add(contact, cc)
    }

    function writeTo(address, account) {
        if (!page.accountChosen && composer.people.length === 0 && accounts.ids.includes(account))
            page.use(account)
        page.add({ name: "", address: address }, false)
    }

    function pickCurrent(cc) {
        const row = list.currentItem as ContactRow
        if (!row)
            return
        page.pick(row, cc)
        search.text = ""
    }

    function toFirstContact() {
        list.currentIndex = list.count > 0 ? 0 : -1
        list.positionViewAtBeginning()
    }

    function follow() {
        if (page.accountChosen)
            return
        const first = composer.people.find(person => person.kind === "to")
        const wanted = first ? contacts.accountOf(first.address) : ""
        if (accounts.ids.includes(wanted))
            page.use(wanted)
        else if (!accounts.ids.includes(page.account))
            page.use(accounts.ids.length > 0 ? accounts.ids[0] : "")
    }

    function use(id) {
        if (page.account === id)
            return
        page.account = id
        composer.from = composer.defaultFrom
    }

    function switchAccount() {
        const ids = accounts.ids
        page.use(ids[(ids.indexOf(page.account) + 1) % ids.length])
        page.accountChosen = composer.people.length > 0
        composer.draftId = ""
        composer.attachments = composer.kept(composer.attachments, "")
    }

    function send() {
        if (composer.canSend)
            page.sendRequested(page.job(), composer.names())
    }

    function job() {
        const opened = draftMail.path.length > 0 && composer.draftId === draftMail.draft.draftId
        return Object.assign(composer.job(), { opened: opened ? draftMail.path : "" })
    }

    function saveDraft() {
        if (composer.canSave)
            page.draftRequested(page.job())
    }

    function discard() {
        if (composer.canDiscard)
            page.discardRequested(page.job())
    }

    function drafted(before, draftId) {
        if (composer.draftId !== before)
            return
        composer.draftId = draftId
        page.accountChosen = true
        composer.attachments = composer.kept(composer.attachments, draftId)
    }

    function undraft(draftId) {
        if (composer.draftId !== draftId)
            return
        composer.draftId = ""
        composer.attachments = composer.kept(composer.attachments, "")
    }

    function openDraft(path) {
        page.openingDraft = true
        draftMail.open(path)
    }

    function continueDraft() {
        if (!page.openingDraft)
            return
        if (draftMail.error.length > 0) {
            page.openingDraft = false
            page.failed(draftMail.error)
            return
        }
        if (draftMail.account.length === 0)
            return
        page.openingDraft = false
        const draft = Object.assign({}, draftMail.draft, { attachments: draftMail.attachments })
        if (draft.parent.length > 0) {
            page.replyContinued(draft.parent, Object.assign(draft, { opened: draftMail.path }))
            return
        }
        page.use(draftMail.account)
        page.accountChosen = true
        composer.restore(draft)
        composer.focusText()
        page.draftOpened()
    }

    function forget(text) {
        if (composer.text !== text)
            return
        page.accountChosen = false
        composer.restore({ from: composer.defaultFrom, people: [], subject: "", text: "" })
        page.follow()
    }

    function restore(draft, account) {
        if (composer.people.length > 0 || composer.text.trim().length > 0)
            return
        page.use(account)
        page.accountChosen = true
        composer.restore(draft)
    }

    Component.onCompleted: accounts.reload()

    onActiveChanged: {
        if (!active)
            return
        contacts.arrange()
        contacts.reload()
        accounts.reload()
        if (composer.people.length > 0)
            composer.focusText()
        else
            search.forceActiveFocus()
    }

    Keys.onEscapePressed: page.closeRequested()

    Backdrop {}

    Column {
        id: widestTag
        opacity: 0
        enabled: false

        Repeater {
            model: accounts.ids

            Tag {
                required property string modelData
                text: modelData
            }
        }
    }

    PageHeader {
        id: header
        title: Strings.newMail
        onBackRequested: page.closeRequested()
    }

    Split {
        anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        category: "compose"
        defaultFirstWidth: 0.4
        defaultSecondHeight: 0.6
        minimumSecondHeight: 0.3
        maximumSecondHeight: 0.85

        SearchInput {
            id: search
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: 16
            placeholderText: Strings.searchContacts
            onTextChanged: contacts.query = text
            Keys.onPressed: (event) => {
                if (event.key === Qt.Key_Down) {
                    list.incrementCurrentIndex()
                } else if (event.key === Qt.Key_Up) {
                    list.decrementCurrentIndex()
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    page.pickCurrent((event.modifiers & Qt.ControlModifier) !== 0)
                } else {
                    return
                }
                event.accepted = true
            }
        }

        ListView {
            id: list
            anchors.top: search.bottom
            anchors.topMargin: 8
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: hint.top
            model: contacts
            clip: true
            currentIndex: 0
            highlightFollowsCurrentItem: false
            boundsBehavior: Flickable.StopAtBounds
            flickDeceleration: 6000
            maximumFlickVelocity: 4000
            ScrollBar.vertical: SlimScrollBar {}

            delegate: ContactRow {
                id: contact
                chosen: composer.has(contact.address)
                tagWidth: widestTag.implicitWidth
                onPicked: (cc) => page.pick(contact, cc)
            }
        }

        WheelArea {
            flickable: list
        }

        Text {
            anchors.centerIn: list
            width: list.width - 48
            visible: list.count === 0 && !contacts.loading
            horizontalAlignment: Text.AlignHCenter
            text: contacts.error.length > 0 ? contacts.error : (contacts.query.length > 0 ? Strings.noMatch : Strings.noContacts)
            color: contacts.error.length > 0 ? Theme.red : Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize + 1
            wrapMode: Text.Wrap
        }

        Text {
            id: hint
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 36
            leftPadding: 20
            verticalAlignment: Text.AlignVCenter
            text: Strings.pickHint
            color: Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize - 1
            elide: Text.ElideRight
        }

        second: Composer {
            id: composer
            anchors.fill: parent
            account: page.account
            tint: Theme[accounts.colors[page.account]] ?? Theme.accent
            canSwitchAccount: accounts.ids.length > 1
            defaultFrom: accounts.senders[page.account] ?? ""
            signature: accounts.signatures[page.account] ?? ""
            placeholder: Strings.mailPlaceholder
            onAccountSwitched: page.switchAccount()
            onFromEdited: page.accountChosen = true
            onPeopleChanged: page.follow()
            allowed: !page.noting
            onSendRequested: page.send()
            onSaveRequested: page.saveDraft()
            onDiscardRequested: page.discard()
            onWarning: (message) => page.failed(message)
        }
    }
}
