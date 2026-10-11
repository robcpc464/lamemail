pragma ComponentBehavior: Bound

import QtCore
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import Lamemail

View {
    id: page
    readonly property bool compact: width < 900
    property bool trayMissing: false
    readonly property list<SettingsSection> sections: [general, mailboxes, assistant, folderKinds, commandLine]
    readonly property string section: (sections.find((entry) => entry.key === memory.section) ?? sections[0]).key
    signal closeRequested()
    signal accountAdded(string name, string message)
    signal accountRemoved(string message)
    signal loginsChanged()
    signal folderAssigned(string name)

    Settings {
        id: memory
        category: "settings"
        property string section
    }

    Folders {
        id: folders
    }

    Manual {
        id: manual
    }

    Accounts {
        id: accounts
        onAddFinished: (ok, name, message) => {
            if (!ok)
                return
            page.accountAdded(name, message)
            page.loginsChanged()
        }
        onFolderAssigned: (name) => page.folderAssigned(name)
        onRemoved: (name, message) => {
            page.takeFocusBack()
            page.accountRemoved(message)
            page.loginsChanged()
        }
        onPasswordStored: page.loginsChanged()
    }

    FolderDialog {
        id: contextDialog
        property string account
        title: Strings.contextChoose
        onAccepted: accounts.setContextFolder(contextDialog.account, selectedFolder)
    }

    function takeFocusBack() {
        if (active)
            page.forceActiveFocus()
    }

    function refresh() {
        if (active && section === folderKinds.key)
            folders.reload()
        if (active && (section === mailboxes.key || section === assistant.key))
            accounts.reload()
    }

    function load() {
        page.refresh()
        if (active && section === assistant.key)
            Ai.reload()
        if (active && section === commandLine.key && manual.commands.length === 0)
            manual.load()
    }

    function step(delta) {
        const keys = page.sections.map((entry) => entry.key)
        const index = keys.indexOf(page.section) + delta
        memory.section = keys[Math.max(0, Math.min(keys.length - 1, index))]
    }

    function toggleFeedKind(kind) {
        const shown = Preferences.feedKinds
        const on = shown.includes(kind)
        Preferences.feedKinds = Styles.mailKinds.filter((each) => each === kind ? !on : shown.includes(each))
    }

    onSectionChanged: page.load()

    onActiveChanged: {
        if (active) {
            page.load()
            page.forceActiveFocus()
        }
    }

    Keys.onEscapePressed: page.closeRequested()
    Keys.onUpPressed: (event) => {
        if (page.activeFocus)
            page.step(-1)
        else
            event.accepted = false
    }
    Keys.onDownPressed: (event) => {
        if (page.activeFocus)
            page.step(1)
        else
            event.accepted = false
    }

    Backdrop {}

    PageHeader {
        id: header
        title: Strings.settings
        onBackRequested: page.closeRequested()
    }

    Item {
        id: sidebar
        anchors.top: header.bottom
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        width: page.compact ? 72 : 232
        Behavior on width { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }

        Column {
            anchors.fill: parent
            anchors.margins: 12
            anchors.topMargin: 20
            spacing: 4

            Repeater {
                model: page.sections

                Rectangle {
                    id: entry
                    required property SettingsSection modelData
                    readonly property bool selected: page.section === modelData.key

                    width: parent.width
                    height: 40
                    radius: Theme.radius
                    color: selected ? Qt.alpha(Theme.accent, 0.16)
                                    : (entryHover.hovered ? Qt.alpha(Theme.foreground, 0.06) : "transparent")
                    Behavior on color { ColorAnimation { duration: 120 } }

                    Icon {
                        id: entryIcon
                        x: 15
                        anchors.verticalCenter: parent.verticalCenter
                        name: entry.modelData.icon
                        size: 18
                        color: entry.selected ? Theme.accent : Theme.muted
                    }

                    Text {
                        anchors.left: entryIcon.right
                        anchors.right: parent.right
                        anchors.leftMargin: 12
                        anchors.rightMargin: 8
                        anchors.verticalCenter: parent.verticalCenter
                        text: entry.modelData.title
                        color: entry.selected ? Theme.foregroundBright : Theme.foreground
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontSize + 1
                        elide: Text.ElideRight
                        opacity: page.compact ? 0 : 1
                        Behavior on opacity { NumberAnimation { duration: 120 } }
                    }

                    HoverHandler {
                        id: entryHover
                        cursorShape: Qt.PointingHandCursor
                    }

                    TapHandler {
                        onTapped: memory.section = entry.modelData.key
                    }
                }
            }
        }

        Column {
            id: about
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.leftMargin: page.compact ? 4 : 27
            anchors.rightMargin: page.compact ? 4 : 12
            anchors.bottomMargin: 14
            spacing: 2

            Repeater {
                model: page.compact ? [Application.version]
                                    : [Strings.wordmark + " " + Application.version, Strings.license,
                                       Strings.noWarranty]

                Text {
                    required property string modelData

                    width: about.width
                    text: modelData
                    color: aboutHover.hovered ? Theme.foreground : Theme.muted
                    Behavior on color { ColorAnimation { duration: 120 } }
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontSize - 2
                    horizontalAlignment: page.compact ? Text.AlignHCenter : Text.AlignLeft
                    elide: Text.ElideRight
                }
            }

            HoverHandler {
                id: aboutHover
                cursorShape: Qt.PointingHandCursor
            }

            Tip {
                visible: aboutHover.hovered && page.compact
                text: [Strings.license, Strings.noWarranty].join(" · ")
            }

            TapHandler {
                onTapped: Qt.openUrlExternally(Strings.licenseLink)
            }
        }

        Rectangle {
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.right: parent.right
            width: 1
            color: Theme.line
            opacity: 0.25
        }
    }

    Item {
        anchors.top: header.bottom
        anchors.bottom: parent.bottom
        anchors.left: sidebar.right
        anchors.right: parent.right
        clip: true

        SettingsSection {
            id: general
            key: "general"
            icon: "settings"
            title: Strings.generalSection
            hint: Strings.generalHint
            active: page.section === key

            Flickable {
                id: generalList
                anchors.fill: parent
                contentHeight: generalRows.height
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: SlimScrollBar {}

                Column {
                    id: generalRows
                    width: generalList.width

                    SettingRow {
                        title: Strings.theme
                        hint: Strings.themeHint

                        Choice {
                            options: ["system", "light", "dark"]
                            labels: [Strings.themeSystem, Strings.themeLight, Strings.themeDark]
                            value: Preferences.theme
                            onChosen: (value) => {
                                Preferences.theme = value
                                Theme.reload()
                            }
                        }
                    }

                    SettingRow {
                        title: Strings.wheel
                        hint: Strings.wheelHint

                        Choice {
                            options: [60, 120, 180, 240, 360]
                            value: Preferences.wheelPixels
                            onChosen: (value) => Preferences.wheelPixels = value
                        }
                    }

                    SettingRow {
                        title: Strings.closeToTray
                        hint: page.trayMissing ? Strings.noTray : Strings.closeToTrayHint

                        Pill {
                            text: Preferences.closeToTray ? Strings.on : Strings.off
                            on: Preferences.closeToTray
                            onClicked: Preferences.closeToTray = !Preferences.closeToTray
                        }
                    }

                    SettingRow {
                        title: Strings.notifications
                        hint: Strings.notificationsHint

                        Pill {
                            text: Preferences.notifications ? Strings.on : Strings.off
                            on: Preferences.notifications
                            onClicked: Preferences.notifications = !Preferences.notifications
                        }
                    }

                    SettingRow {
                        title: Strings.shadows
                        hint: Preferences.shadowsAvailable ? Strings.shadowsHint : Strings.shadowsMissing

                        Pill {
                            readonly property bool shown: Preferences.shadows && Preferences.shadowsAvailable
                            enabled: Preferences.shadowsAvailable
                            opacity: enabled ? 1 : 0.4
                            text: shown ? Strings.on : Strings.off
                            on: shown
                            onClicked: Preferences.shadows = !Preferences.shadows
                        }
                    }

                    SettingRow {
                        title: Strings.flaggedFirst
                        hint: Strings.flaggedFirstHint

                        Pill {
                            text: Preferences.flaggedFirst ? Strings.on : Strings.off
                            on: Preferences.flaggedFirst
                            onClicked: Preferences.flaggedFirst = !Preferences.flaggedFirst
                        }
                    }

                    SettingRow {
                        title: Strings.downloadFolder
                        hint: Strings.downloadFolderHint

                        Field {
                            width: 280
                            text: Preferences.downloadFolder
                            placeholderText: Preferences.defaultDownloadFolder
                            onEditingFinished: Preferences.downloadFolder = text
                        }
                    }
                }
            }

            WheelArea {
                flickable: generalList
            }
        }

        SettingsSection {
            id: mailboxes
            key: "accounts"
            icon: "user"
            title: Strings.accountsSection
            hint: Strings.accountsHint
            error: accounts.error
            active: page.section === key

            ListView {
                id: accountList
                anchors.fill: parent
                model: accounts
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                delegate: AccountSetting {
                    accounts: accounts
                    onClosed: page.takeFocusBack()
                }
                footer: NewAccount {
                    accounts: accounts
                    onClosed: page.takeFocusBack()
                }
                ScrollBar.vertical: SlimScrollBar {}
            }

            WheelArea {
                flickable: accountList
            }
        }

        SettingsSection {
            id: assistant
            key: "ai"
            icon: "sparkles"
            title: Strings.aiSection
            hint: Strings.aiHint
            error: Ai.error
            active: page.section === key

            Flickable {
                id: aiList
                anchors.fill: parent
                contentHeight: aiRows.height
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: SlimScrollBar {}

                Column {
                    id: aiRows
                    width: aiList.width

                    Item {
                        width: parent.width
                        height: privacy.height + 16

                        Rectangle {
                            id: privacy
                            x: 24
                            y: 8
                            width: parent.width - 48
                            height: privacyRow.implicitHeight + 28
                            radius: Theme.radius
                            color: Qt.alpha(Theme.yellow, 0.1)
                            border.width: 1
                            border.color: Qt.alpha(Theme.yellow, 0.45)

                            Row {
                                id: privacyRow
                                x: 14
                                y: 14
                                width: parent.width - 28
                                spacing: 12

                                Icon {
                                    name: "junk"
                                    size: 18
                                    color: Theme.yellow
                                }

                                Column {
                                    width: parent.width - 30
                                    spacing: 4

                                    Text {
                                        text: Strings.aiPrivacyTitle
                                        color: Theme.foregroundBright
                                        font.family: Theme.fontFamily
                                        font.pixelSize: Theme.fontSize + 1
                                        font.bold: true
                                    }

                                    Text {
                                        width: parent.width
                                        text: Strings.aiPrivacy
                                        color: Theme.foreground
                                        font.family: Theme.fontFamily
                                        font.pixelSize: Theme.fontSize
                                        wrapMode: Text.Wrap
                                        lineHeight: 1.15
                                    }
                                }
                            }
                        }
                    }

                    SettingHeading {
                        title: Strings.aiModel
                        hint: Strings.aiModelHint
                    }

                    Repeater {
                        model: Ai.programs

                        SettingRow {
                            id: program
                            required property var modelData
                            readonly property bool claude: modelData.program === "claude-code"
                            readonly property string command: modelData.command
                            readonly property string model: modelData.ollama
                            readonly property bool usable: claude ? modelData.running : model.length > 0
                            readonly property bool active: claude ? Ai.active === "claude-code"
                                                                  : usable && Ai.active === "ollama" && model === Ai.ollama

                            opacity: modelData.found ? 1 : 0.5
                            title: modelData.name
                            hint: program.command.length > 0 ? program.command
                                  : ({ "claude-code": Strings.aiAboutClaude, ollama: Strings.aiAboutOllama })[modelData.program] ?? ""
                            link: modelData.url

                            Row {
                                Pill {
                                    visible: program.usable
                                    text: program.active ? Strings.on : Strings.off
                                    on: program.active
                                    onClicked: {
                                        if (program.active)
                                            Ai.remove()
                                        else
                                            Ai.choose(program.modelData.program, program.model)
                                    }
                                }

                                Text {
                                    visible: !program.usable
                                    text: !program.modelData.found ? Strings.aiNotFound
                                          : !program.modelData.running ? Strings.aiNotRunning : Strings.aiNoModel
                                    color: Theme.muted
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontSize - 1
                                }
                            }
                        }
                    }

                    SettingRow {
                        id: custom
                        readonly property bool edited: programField.text.trim() !== Ai.custom
                        readonly property bool active: Ai.active === "custom"

                        title: Strings.aiProgram
                        note: Strings.aiProgramHint

                        Row {
                            spacing: 8

                            Field {
                                id: programField
                                width: Math.min(520, page.width * 0.5)
                                text: Ai.custom
                                placeholderText: Strings.aiProgramPlaceholder
                            }

                            Pill {
                                anchors.verticalCenter: programField.verticalCenter
                                visible: custom.active && custom.edited && programField.text.trim().length > 0
                                text: Strings.save
                                on: true
                                onClicked: Ai.choose("custom", programField.text)
                            }

                            Pill {
                                readonly property bool possible: custom.active || programField.text.trim().length > 0

                                anchors.verticalCenter: programField.verticalCenter
                                enabled: possible
                                opacity: possible ? 1 : 0.4
                                text: custom.active ? Strings.on : Strings.off
                                on: custom.active
                                onClicked: custom.active ? Ai.remove() : Ai.choose("custom", programField.text)
                            }
                        }

                        Connections {
                            target: Ai

                            function onChanged() {
                                programField.text = Ai.custom
                            }
                        }
                    }

                    SettingHeading {
                        title: Strings.context
                        hint: Strings.contextHint
                    }

                    SettingRow {
                        title: Strings.contextLimit
                        hint: Strings.contextLimitHint

                        Choice {
                            options: Ai.contextLimits
                            labels: Ai.contextLimits.map((chars) => chars.toLocaleString(Qt.locale(), "f", 0))
                            value: Ai.contextLimit
                            onChosen: (value) => Ai.setContextLimit(value)
                        }
                    }

                    Repeater {
                        model: accounts

                        SettingRow {
                            id: contextRow
                            required property string name
                            required property string contextFolder
                            readonly property bool chosen: contextFolder.length > 0

                            title: contextRow.name
                            hint: chosen ? contextRow.contextFolder : Strings.contextNone

                            Row {
                                spacing: 8

                                Pill {
                                    text: Strings.contextChoose
                                    on: !contextRow.chosen
                                    onClicked: {
                                        contextDialog.account = contextRow.name
                                        contextDialog.open()
                                    }
                                }

                                Pill {
                                    visible: contextRow.chosen
                                    text: Strings.remove
                                    onClicked: accounts.setContextFolder(contextRow.name, "")
                                }
                            }
                        }
                    }
                }
            }

            WheelArea {
                flickable: aiList
            }
        }

        SettingsSection {
            id: folderKinds
            key: "feed"
            icon: "list"
            title: Strings.feedSection
            hint: Strings.feedHint
            error: folders.error
            active: page.section === key

            ListView {
                id: folderList
                anchors.fill: parent
                model: folders
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                header: SettingRow {
                    width: folderList.width
                    title: Strings.feedKinds
                    hint: Strings.feedKindsHint

                    Row {
                        spacing: 6

                        Repeater {
                            model: Styles.mailKinds

                            Pill {
                                id: kindPill
                                required property string modelData
                                icon: Styles.kindIcon(modelData)
                                tip: Styles.kindLabel(modelData)
                                on: Preferences.feedKinds.includes(modelData)
                                onClicked: page.toggleFeedKind(kindPill.modelData)
                            }
                        }
                    }
                }
                delegate: FolderSetting {}
                ScrollBar.vertical: SlimScrollBar {}
            }

            WheelArea {
                flickable: folderList
            }
        }

        SettingsSection {
            id: commandLine
            key: "commands"
            icon: "terminal"
            title: Strings.commandsSection
            hint: Strings.commandsHint
            error: manual.error
            active: page.section === key

            ListView {
                id: commandList
                anchors.fill: parent
                model: manual.commands
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                delegate: CommandHelp {}
                ScrollBar.vertical: SlimScrollBar {}
            }

            WheelArea {
                flickable: commandList
            }
        }
    }
}
