pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import Qt.labs.platform as Platform
import Lamemail

Window {
    id: root
    width: 1000
    height: 720
    minimumWidth: 480
    minimumHeight: 320
    title: Strings.wordmark
    color: Theme.background

    required property bool startHidden
    property var views: ["feed"]
    readonly property string view: views[views.length - 1]
    property var pending: null
    property bool quitting: false
    property bool exiting: false
    property bool unnoticed: false
    property var lastSync: null
    property var undoMoves: []
    property var undoing: []
    property bool pushAfterUndo: false
    property var unsent: ({})
    readonly property bool toTray: (tray.object as Platform.SystemTrayIcon)?.available ?? false
    readonly property bool searching: feed.query.trim().length > 0

    function enter(name) {
        if (root.view !== name)
            root.views = root.views.concat([name])
    }

    function back() {
        if (root.views.length > 1)
            root.views = root.views.slice(0, -1)
    }

    onViewChanged: if (root.view === "feed") list.forceActiveFocus()
    onActiveChanged: if (root.active) root.unnoticed = false

    function present() {
        root.show()
        root.raise()
        root.requestActivate()
    }

    function toggle() {
        if (root.visible && root.active)
            root.hide()
        else
            root.present()
    }

    function writeTo(address) {
        composePage.writeTo(address, mailPage.account)
        root.enter("compose")
    }

    function reveal(path) {
        root.present()
        if (path.length > 0)
            root.showMail(path)
    }

    function showMail(path) {
        feed.markSeen(path, true)
        mailPage.show(path)
        root.enter("mail")
    }

    function quit() {
        root.exiting = true
        root.close()
    }

    onClosing: (close) => {
        const hide = root.toTray && !root.exiting && !root.quitting
        root.exiting = false
        if (hide) {
            close.accepted = false
            root.hide()
            return
        }
        root.release()
        if ((sender.noting > 0 || journal.sending > 0) && !root.quitting) {
            close.accepted = false
            root.quitting = true
            toast.show({ icon: "outbox", text: Strings.closingAfterSend })
            if (sender.noting === 0)
                journal.push()
        } else {
            Qt.quit()
        }
    }

    function quitWhenSent() {
        if (root.quitting && sender.noting === 0 && !journal.pushing)
            Qt.quit()
    }

    function requestSend(job, recipients) {
        root.hold(job, "outbox", Strings.sending.replace("{}", () => recipients))
    }

    function requestDiscard(job) {
        root.hold(Object.assign(job, { discard: true }), "trash", Strings.discarding)
    }

    function requestDelete(path) {
        if (root.pending?.path !== path)
            root.hold({ path: path, deleteForGood: true }, "trash", root.named(Strings.deleting, path))
    }

    function hold(job, icon, text) {
        if (job.draftId?.length > 0 && root.pending?.draftId === job.draftId) {
            holdDelay.stop()
            root.pending = null
        }
        root.release()
        root.settleMoves()
        root.pending = job
        holdDelay.restart()
        root.back()
        toast.show({ icon: icon, text: text, action: Strings.undo, duration: holdDelay.interval })
    }

    function release() {
        if (!root.pending)
            return
        holdDelay.stop()
        if (root.undoMoves.length === 0)
            toast.dismiss()
        const job = root.pending
        root.pending = null
        if (job.discard)
            sender.discard(job)
        else if (job.deleteForGood)
            sender.deleteForGood(job.path)
        else
            sender.send(job)
    }

    function undo() {
        if (root.undoMoves.length > 0) {
            const moves = root.undoMoves
            toast.dismiss()
            root.settleMoves()
            root.undoing = root.undoing.concat(moves.map(move => move.path))
            moves.forEach(move => feed.move(move.path, "/" + move.folder))
        } else if (root.pending) {
            root.undoHeld()
        }
    }

    function named(text, path) {
        const chars = Array.from(feed.subject(path).trim())
        const shown = chars.length > 40 ? chars.slice(0, 39).join("").trim() + "…" : chars.join("")
        return text.replace("{}", () => shown.length > 0 ? shown : "—")
    }

    function offerUndoMove(path, from, folder, target) {
        const origin = root.undoMoves.find(move => move.path === path)?.folder ?? from
        const others = root.undoMoves.filter(move => move.path !== path)
        const shown = target.startsWith("/") ? "folder" : target
        root.undoMoves = folder === origin ? others : others.concat([{ path: path, folder: origin, target: shown }])
        if (root.undoMoves.length === 0) {
            toast.dismiss()
            root.settleMoves()
            return
        }
        undoWindow.restart()
        const single = root.undoMoves.length === 1 ? root.undoMoves[0].target : ""
        const counts = root.undoMoves.reduce((all, move) => Object.assign(all, { [move.target]: (all[move.target] ?? 0) + 1 }), {})
        const done = ({ archive: Strings.archived, trash: Strings.trashed, inbox: Strings.movedToInbox, folder: Strings.moved })[single]
        toast.show({
            icon: single,
            text: done ? root.named(done, root.undoMoves[0].path) : "",
            parts: single ? [] : Object.keys(counts).map(kind => ({ icon: kind, text: String(counts[kind]) })),
            action: Strings.undo,
            duration: undoWindow.interval
        })
    }

    function settleMoves() {
        undoWindow.stop()
        root.undoMoves = []
        if (root.pushAfterUndo) {
            root.pushAfterUndo = false
            journal.push()
        }
    }

    function pushSoon() {
        if (undoWindow.running)
            root.pushAfterUndo = true
        else
            journal.push()
    }

    function undoHeld() {
        if (!root.pending)
            return
        holdDelay.stop()
        const job = root.pending
        root.pending = null
        toast.dismiss()
        if (job.deleteForGood)
            return
        if (job.path) {
            mailPage.show(job.path)
            root.enter("mail")
        } else {
            root.enter("compose")
        }
    }

    function noted(error) {
        if (error.length === 0)
            return true
        root.quitting = false
        toast.show({ icon: "junk", ok: false, text: error })
        return false
    }

    Feed {
        id: feed
        flaggedFirst: Preferences.flaggedFirst
        kinds: bar.kind.length > 0 ? [bar.kind] : Preferences.feedKinds
        onActionFailed: (path, error) => {
            root.undoing = root.undoing.filter(undone => undone !== path)
            toast.show({ icon: "junk", ok: false, text: error })
            mailPage.refresh()
        }
        onMoved: (path, from, folder, target) => {
            const undone = root.undoing.indexOf(path)
            if (undone >= 0)
                root.undoing = root.undoing.filter((other, at) => at !== undone)
            else
                root.offerUndoMove(path, from, folder, target)
        }
    }

    Syncer {
        id: syncer
        onFinished: (ok, counts, error) => {
            const parts = (pairs) => pairs.filter(([, count]) => count > 0)
                                          .map(([icon, count]) => ({ icon: icon, text: String(count) }))
            if (!ok) {
                toast.show({ icon: "junk", ok: false, text: error, parts: counts.failed > 1 ? parts([["user", counts.failed]]) : [] })
            } else {
                const shown = parts([["user", counts.accounts], ["plus", counts.added], ["minus", counts.removed], ["drafts", counts.updated]])
                toast.show({ icon: "sync", parts: shown, text: shown.length > 1 ? "" : Strings.upToDate })
                root.lastSync = new Date()
            }
            feed.reload()
        }
    }

    Sender {
        id: sender
        onFinished: (job, messageId, error) => {
            if (!root.noted(error))
                return
            root.unsent[messageId] = job
            if (job.path)
                mailPage.forget(job.path, job.text)
            else
                composePage.forget(job.text)
            root.pushSoon()
        }
        onSaved: (job, messageId, error) => {
            if (!root.noted(error))
                return
            if (job.path)
                mailPage.drafted(job.path, job.draftId, messageId)
            else
                composePage.drafted(job.draftId, messageId)
            toast.show({ icon: "save", text: Strings.draftSaved })
            root.pushSoon()
        }
        onDiscarded: (job, error) => {
            if (!root.noted(error))
                return
            if (job.path) {
                mailPage.forget(job.path, job.text)
                mailPage.drafted(job.path, job.draftId, "")
            } else {
                composePage.forget(job.text)
                composePage.undraft(job.draftId)
            }
            root.pushSoon()
        }
        onDeletedForGood: (path, error) => {
            if (root.noted(error))
                root.pushSoon()
        }
    }

    Timer {
        id: holdDelay
        interval: 5000
        onTriggered: root.release()
    }

    Timer {
        id: undoWindow
        interval: 5000
        onTriggered: root.settleMoves()
    }

    MailWatch {
        id: mailWatch
        onSynced: root.lastSync = new Date()
        onArrived: (mails) => {
            if (root.active)
                return
            root.unnoticed = true
            if (Preferences.notifications)
                notifier.announce(mails)
        }
        onStopped: (message) => toast.show({ icon: "junk", ok: false, text: message })
    }

    Mailto {
        onRequested: (address) => root.writeTo(address)
    }

    Notifier {
        id: notifier
        onOpened: (path) => root.reveal(path)
    }

    Journal {
        id: journal
        onDropped: (reason, messageId) => {
            const job = root.unsent[messageId]
            if (job?.path)
                mailPage.restore(job.path, job.draft)
            else if (job)
                composePage.restore(job.draft, job.account)
            delete root.unsent[messageId]
            toast.show({ icon: "junk", ok: false, text: reason })
        }
        onSent: (messageId, message, complete) => {
            delete root.unsent[messageId]
            toast.show({ icon: complete ? "outbox" : "junk", ok: complete, text: message })
        }
        onFailureChanged: {
            if (journal.failure.length > 0 && Object.keys(root.unsent).length > 0)
                toast.show({ icon: "junk", ok: false, text: journal.failure })
        }
        onChanged: {
            if (journal.sending === 0 && !journal.pushing)
                root.unsent = {}
        }
        onPushingChanged: Qt.callLater(root.quitWhenSent)
    }

    Watcher {
        id: watcher
        onChanged: {
            journal.reload()
            feed.reload()
            mailPage.refresh()
            composePage.refresh()
            settings.refresh()
        }
        onThemeChanged: Theme.reload()
    }

    Timer {
        interval: 60000
        running: true
        repeat: true
        onTriggered: feed.refreshTimes()
    }

    Instantiator {
        id: tray
        active: Preferences.closeToTray

        Platform.SystemTrayIcon {
            visible: true
            icon.source: root.unnoticed ? "image://tray/" + String(Theme.accent).slice(1) : "qrc:/lamemail.svg"
            tooltip: Strings.wordmark
            onActivated: (reason) => {
                if (reason === Platform.SystemTrayIcon.Trigger)
                    root.toggle()
            }

            menu: Platform.Menu {
                Platform.MenuItem {
                    text: Strings.open
                    onTriggered: root.present()
                }

                Platform.MenuItem {
                    text: Strings.quit
                    onTriggered: root.quit()
                }
            }
        }
    }

    Component.onCompleted: {
        if (!root.startHidden || !root.toTray)
            root.show()
        Ai.reload()
        feed.reload()
        journal.reload()
        watcher.start()
        mailWatch.start()
    }

    View {
        id: feedLayer
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: statusBar.top
        active: root.view === "feed"

        TopBar {
            id: bar
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            searching: search.open
            filtering: !root.searching
            onSearchRequested: {
                if (search.open) {
                    search.hide()
                    list.forceActiveFocus()
                } else {
                    search.show()
                }
            }
            syncing: syncer.running || mailWatch.syncing.length > 0
            onSyncRequested: syncer.sync()
            onSettingsRequested: root.enter("settings")
            onComposeRequested: root.enter("compose")
        }

        SearchBar {
            id: search
            anchors.top: bar.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            hits: root.searching ? Strings.hits.replace("{}", feed.count) : ""
            onSearched: (text) => feed.query = text
            onListRequested: {
                if (list.currentIndex < 0 && list.count > 0)
                    list.currentIndex = 0
                list.forceActiveFocus()
            }
        }

        Rectangle {
            id: banner
            anchors.top: search.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            height: feed.error.length > 0 ? bannerText.implicitHeight + 20 : 0
            clip: true
            color: Qt.alpha(Theme.red, 0.15)
            Behavior on height { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }

            Text {
                id: bannerText
                anchors.centerIn: parent
                width: parent.width - 48
                text: feed.error
                color: Theme.red
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize + 1
                wrapMode: Text.Wrap
            }
        }

        ListView {
            id: list
            anchors.top: banner.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            model: feed
            clip: true
            focus: true
            currentIndex: -1
            keyNavigationEnabled: true
            boundsBehavior: Flickable.StopAtBounds
            flickDeceleration: 6000
            maximumFlickVelocity: 4000
            highlightFollowsCurrentItem: false
            cacheBuffer: Math.max(0, height)
            section.property: "day"
            section.labelPositioning: ViewSection.InlineLabels | ViewSection.CurrentLabelAtStart
            section.delegate: DayHeader {
                height: list.dayHeight
            }

            delegate: MailRow {
                id: row
                locked: root.pending?.path === row.path || root.pending?.opened === row.path
                        || root.undoMoves.some(move => move.path === row.path)
                waiting: journal.leaving.includes(row.path) || journal.answering.includes(row.path)
                onClicked: {
                    list.currentIndex = row.index
                    list.openCurrent()
                }
                onStarClicked: feed.markFlagged(row.path, !row.flagged)
            }

            function openCurrent() {
                const row = list.currentItem as MailRow
                if (row?.locked)
                    return
                if (row?.kind === "drafts")
                    composePage.openDraft(row.path)
                else if (row)
                    root.showMail(row.path)
            }

            function moveCurrent(key) {
                const row = list.currentItem as MailRow
                const trash = key === Qt.Key_Delete
                if (trash && row?.kind === "trash") {
                    root.requestDelete(row.path)
                    list.passBy(row.path)
                    return
                }
                const target = row ? Styles.moves(row.kind).find(move => (move === "trash") === trash) : undefined
                if (!target)
                    return
                feed.move(row.path, target)
                if (trash)
                    list.passBy(row.path)
            }

            function passBy(path) {
                if ((list.currentItem as MailRow)?.path === path)
                    list.step(1)
            }

            function markCurrent(key) {
                const row = list.currentItem as MailRow
                if (!row)
                    return
                if (key === Qt.Key_U)
                    feed.markSeen(row.path, !row.seen)
                else
                    feed.markFlagged(row.path, !row.flagged)
            }

            ScrollBar.vertical: SlimScrollBar {}

            property real scrollTarget: 0
            readonly property real dayHeight: 36
            readonly property int wheelRows: Math.max(1, Math.round(Preferences.wheelPixels / 60))

            function scrollTo(y) {
                const top = list.originY
                const bottom = Math.max(top, list.originY + list.contentHeight - list.height)
                list.scrollTarget = Math.max(top, Math.min(bottom, y))
                list.flickTo(Qt.point(list.contentX, list.scrollTarget))
            }

            function scrollPage(direction) {
                const from = list.flicking ? list.scrollTarget : list.contentY
                const top = list.itemAtIndex(list.rowAt(from))
                const rows = top ? Math.max(1, Math.floor((list.height - list.dayHeight) / top.height) - 1) : 1
                list.scrollTo(list.rowsFrom(from, direction * rows))
            }

            function rowAt(y) {
                const line = y + list.dayHeight
                const index = list.indexAt(0, line)
                return index >= 0 ? index : list.indexAt(0, line + list.dayHeight)
            }

            function rowsFrom(y, rows) {
                const index = list.rowAt(y)
                const here = list.itemAtIndex(index)
                if (!here)
                    return y
                const cut = rows < 0 && here.y < y + list.dayHeight - 1 ? 1 : 0
                let to = Math.max(0, Math.min(list.count - 1, index + rows + cut))
                while (to !== index && !list.itemAtIndex(to))
                    to += rows > 0 ? -1 : 1
                return list.itemAtIndex(to).y - list.dayHeight
            }

            function step(direction) {
                if (direction > 0)
                    list.incrementCurrentIndex()
                else
                    list.decrementCurrentIndex()
                const item = list.currentItem
                const from = list.flicking ? list.scrollTarget : list.contentY
                if (!item)
                    return
                if (item.y < from + list.dayHeight) {
                    list.scrollTo(item.y - list.dayHeight)
                } else if (item.y + item.height > from + list.height) {
                    let target = from
                    for (let rows = 1; item.y + item.height > target + list.height; ++rows) {
                        const next = list.rowsFrom(from, rows)
                        if (next === target)
                            break
                        target = next
                    }
                    list.scrollTo(target)
                }
            }

            Keys.onPressed: (event) => {
                const ctrl = (event.modifiers & ~Qt.KeypadModifier) === Qt.ControlModifier && !event.isAutoRepeat
                if (event.key === Qt.Key_Down || event.key === Qt.Key_Up) {
                    list.step(event.key === Qt.Key_Down ? 1 : -1)
                } else if (event.key === Qt.Key_PageDown) {
                    list.scrollPage(1)
                } else if (event.key === Qt.Key_PageUp) {
                    list.scrollPage(-1)
                } else if (event.key === Qt.Key_Home) {
                    list.currentIndex = 0
                    list.cancelFlick()
                    list.positionViewAtBeginning()
                } else if (event.key === Qt.Key_End) {
                    list.currentIndex = list.count - 1
                    list.cancelFlick()
                    list.positionViewAtEnd()
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    list.openCurrent()
                } else if (ctrl && (event.key === Qt.Key_U || event.key === Qt.Key_D)) {
                    list.markCurrent(event.key)
                } else if (ctrl && (event.key === Qt.Key_E || event.key === Qt.Key_Delete)) {
                    list.moveCurrent(event.key)
                } else if (event.key === Qt.Key_Escape && (root.pending || root.undoMoves.length > 0) && !event.isAutoRepeat) {
                    root.undo()
                } else if (event.key === Qt.Key_Escape && search.open) {
                    search.hide()
                } else if (event.text === "/") {
                    search.show()
                } else {
                    return
                }
                event.accepted = true
            }

            add: feed.streaming ? fadeIn : slideIn

            Transition {
                id: fadeIn
                SequentialAnimation {
                    PropertyAction { property: "opacity"; value: 0 }
                    PauseAnimation { duration: Math.min(fadeIn.ViewTransition.index, 25) * 18 }
                    NumberAnimation { property: "opacity"; to: 1; duration: 220; easing.type: Easing.OutCubic }
                }
            }

            Transition {
                id: slideIn
                SequentialAnimation {
                    PropertyAction { property: "opacity"; value: 0 }
                    ParallelAnimation {
                        NumberAnimation { property: "opacity"; to: 1; duration: 220; easing.type: Easing.OutCubic }
                        NumberAnimation {
                            property: "y"
                            from: slideIn.ViewTransition.destination.y - 12
                            to: slideIn.ViewTransition.destination.y
                            duration: 220
                            easing.type: Easing.OutCubic
                        }
                    }
                }
            }

            remove: Transition {
                ParallelAnimation {
                    NumberAnimation { property: "opacity"; to: 0; duration: 180 }
                    NumberAnimation { property: "height"; to: 0; duration: 180; easing.type: Easing.InCubic }
                }
            }

            displaced: Transition {
                NumberAnimation { property: "y"; duration: 200; easing.type: Easing.OutCubic }
            }
        }

        WheelArea {
            flickable: list
            step: (from, notches) => list.rowsFrom(from, notches * list.wheelRows)
        }

        EmptyState {
            anchors.centerIn: list
            visible: feed.count === 0 && (!feed.loading || root.searching)
            error: feed.error
            searching: root.searching
            kind: bar.kind
        }
    }

    MailPage {
        id: mailPage
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: statusBar.top
        active: root.view === "mail"
        noting: sender.noting > 0
        onCloseRequested: root.back()
        onComposeRequested: root.enter("compose")
        onSettingsRequested: root.enter("settings")
        onSendRequested: (job, recipients) => root.requestSend(job, recipients)
        onDraftRequested: (job) => sender.save(job)
        onDiscardRequested: (job) => root.requestDiscard(job)
        onDeleteRequested: (path) => {
            root.requestDelete(path)
            list.passBy(path)
        }
        onFlagRequested: (path, flagged) => feed.markFlagged(path, flagged)
        onMoveRequested: (path, target) => {
            feed.move(path, target)
            root.back()
            if (target === "trash")
                list.passBy(path)
        }
        onSeenRequested: (path, seen) => feed.markSeen(path, seen)
        onFailed: (error) => toast.show({ icon: "junk", ok: false, text: error })
        onAttachmentsDownloaded: (count, folder) => toast.show({
            icon: "download",
            parts: [{ icon: "attachment", text: String(count) }],
            text: folder
        })
    }

    ComposePage {
        id: composePage
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: statusBar.top
        active: root.view === "compose"
        noting: sender.noting > 0
        onCloseRequested: root.back()
        onSendRequested: (job, recipients) => root.requestSend(job, recipients)
        onDraftRequested: (job) => sender.save(job)
        onDiscardRequested: (job) => root.requestDiscard(job)
        onFailed: (error) => toast.show({ icon: "junk", ok: false, text: error })
        onDraftOpened: root.enter("compose")
        onReplyContinued: (path, draft) => {
            mailPage.continueDraft(path, draft)
            root.showMail(path)
        }
    }

    SettingsPage {
        id: settings
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: statusBar.top
        active: root.view === "settings"
        trayMissing: Preferences.closeToTray && !root.toTray
        onCloseRequested: root.back()
        onQuitRequested: root.quit()
        onAccountAdded: (name, message) => {
            toast.show({ icon: "user", text: message })
            syncer.sync([name])
        }
        onAccountRemoved: (message) => toast.show({ icon: "trash", text: message })
        onFolderAssigned: (name) => syncer.sync([name])
        onLoginsChanged: mailWatch.restart()
    }

    StatusBar {
        id: statusBar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        syncing: mailWatch.syncing
        syncBusy: syncer.running
        progress: syncer.running ? syncer.progress : mailWatch.progress
        sending: journal.sending
        sendFailure: journal.failure
        pending: journal.count
        pushing: journal.pushing
        failing: mailWatch.failing
        failures: mailWatch.failures
        lastSync: root.lastSync
        unread: feed.unread
    }

    Toast {
        id: toast
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: statusBar.top
        anchors.bottomMargin: 16
        actionKey: root.view === "feed" ? Strings.undoKey : ""
        onActionTriggered: root.undo()
    }

    Shortcut {
        sequences: ["Ctrl+R", "F5"]
        onActivated: syncer.sync()
    }

    Shortcut {
        sequences: [StandardKey.Undo]
        enabled: root.pending !== null || root.undoMoves.length > 0
        onActivated: root.undo()
    }

    Shortcut {
        sequence: "Ctrl+,"
        onActivated: root.enter("settings")
    }

    Shortcut {
        sequence: "Ctrl+N"
        onActivated: root.enter("compose")
    }

    Shortcut {
        sequence: "Ctrl+Q"
        onActivated: root.quit()
    }
}
