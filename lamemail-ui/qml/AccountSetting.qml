pragma ComponentBehavior: Bound

import QtQuick
import Lamemail

Item {
    id: item

    required property string name
    required property string host
    required property int port
    required property string username
    required property string smtpHost
    required property int smtpPort
    required property bool copyToSent
    required property string color
    required property string signature
    required property bool archiveByYear
    required property string password
    required property var kinds
    required property var folders
    required property bool busy
    required property string status
    required property bool statusOk
    required property Accounts accounts
    property bool open: false
    property bool confirming: false
    property bool purge: false
    signal closed()

    width: ListView.view ? ListView.view.width : 0
    height: head.height + editor.height

    onSignatureChanged: signatureField.text = signature

    onOpenChanged: {
        confirming = false
        purge = false
        smtpHostField.text = smtpHost
        smtpPortField.text = smtpPort > 0 ? String(smtpPort) : ""
        signatureField.text = signature
        passwordField.text = ""
        if (!open)
            closed()
    }

    Connections {
        target: item.accounts

        function onPasswordStored(name) {
            if (name === item.name)
                passwordField.text = ""
        }
    }

    Item {
        id: head
        width: parent.width
        height: 68

        Tag {
            id: tag
            anchors.left: parent.left
            anchors.leftMargin: 24
            anchors.verticalCenter: parent.verticalCenter
            text: item.name
            tint: Theme[item.color] ?? Theme.accent
        }

        Column {
            anchors.left: parent.left
            anchors.right: actions.left
            anchors.leftMargin: 24 + Math.max(tag.width, 84) + 14
            anchors.rightMargin: 16
            anchors.verticalCenter: parent.verticalCenter
            spacing: 3

            Text {
                width: parent.width
                text: item.username
                color: Theme.foreground
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize + 1
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                text: item.host + ":" + item.port + "  ·  "
                      + (item.smtpHost.length > 0 ? Strings.sendingVia.replace("{}", item.smtpHost + ":" + item.smtpPort)
                                                  : Strings.noSmtp)
                color: Theme.muted
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize - 1
                elide: Text.ElideRight
            }
        }

        Row {
            id: actions
            anchors.right: parent.right
            anchors.rightMargin: 24
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8

            Text {
                anchors.verticalCenter: parent.verticalCenter
                width: Math.min(implicitWidth, 320)
                visible: !item.busy && text.length > 0
                text: item.status
                color: item.statusOk ? Theme.green : Theme.red
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize - 1
                elide: Text.ElideRight
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: item.password === "missing" || item.password === "unknown"
                text: item.password === "missing" ? Strings.passwordMissing : Strings.passwordUnknown
                color: Theme.red
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize - 1
            }

            Pill {
                anchors.verticalCenter: parent.verticalCenter
                text: Strings.test
                onClicked: item.accounts.test(item.name)
            }

            Pill {
                anchors.verticalCenter: parent.verticalCenter
                text: Strings.edit
                on: item.open
                onClicked: item.open = !item.open
            }
        }

        BusyLine {
            running: item.busy
        }
    }

    Rectangle {
        anchors.fill: editor
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        radius: Theme.radius
        color: Qt.alpha(Theme.foreground, 0.035)
        opacity: editor.opacity
    }

    Fold {
        id: editor
        anchors.top: head.bottom
        open: item.open

        SettingRow {
            title: Strings.color

            Row {
                spacing: 4

                Repeater {
                    model: Styles.colors

                    Swatch {
                        id: swatch
                        required property string modelData
                        colorName: modelData
                        chosen: modelData === item.color
                        onClicked: item.accounts.setColor(item.name, swatch.modelData)
                    }
                }
            }
        }

        SettingRow {
            title: Strings.smtpServer
            hint: Strings.smtpHint

            Row {
                spacing: 8

                Field {
                    id: smtpHostField
                }

                PortField {
                    id: smtpPortField
                }

                Pill {
                    anchors.verticalCenter: parent.verticalCenter
                    text: Strings.save
                    on: true
                    visible: smtpHostField.text.trim().length > 0
                             && (smtpPortField.text.length === 0 || smtpPortField.acceptableInput)
                             && (smtpHostField.text.trim() !== item.smtpHost || smtpPortField.text !== String(item.smtpPort))
                    onClicked: item.accounts.setSmtp(item.name, smtpHostField.text.trim(), smtpPortField.text)
                }
            }
        }

        SettingRow {
            title: Strings.password
            hint: Strings.passwordHint

            Row {
                spacing: 8

                Field {
                    id: passwordField
                    echoMode: TextInput.Password
                }

                Pill {
                    anchors.verticalCenter: parent.verticalCenter
                    text: Strings.save
                    on: true
                    visible: passwordField.text.length > 0
                    onClicked: item.accounts.setPassword(item.name, passwordField.text)
                }
            }
        }

        SettingRow {
            visible: item.smtpHost.length > 0
            title: Strings.copyToSent
            hint: Strings.copyToSentHint

            Pill {
                text: item.copyToSent ? Strings.on : Strings.off
                on: item.copyToSent
                onClicked: item.accounts.setCopy(item.name, !item.copyToSent)
            }
        }

        SettingRow {
            title: Strings.signature
            hint: Strings.signatureHint

            Row {
                spacing: 8

                TextBox {
                    id: signatureField
                }

                Pill {
                    anchors.bottom: signatureField.bottom
                    text: Strings.save
                    on: true
                    visible: signatureField.text.trim() !== item.signature
                    onClicked: item.accounts.setSignature(item.name, signatureField.text)
                }
            }
        }

        SettingRow {
            title: Strings.archiveByYear
            hint: Strings.archiveByYearHint

            Pill {
                text: item.archiveByYear ? Strings.on : Strings.off
                on: item.archiveByYear
                onClicked: item.accounts.setArchiveByYear(item.name, !item.archiveByYear)
            }
        }

        SettingRow {
            title: Strings.defaultFolders
            hint: Strings.defaultFoldersHint
            divided: false

            Text {
                readonly property var missing: Styles.kinds.filter(kind => kind !== "other" && !item.kinds[kind])
                visible: item.folders.length > 0 && missing.length > 0
                text: Strings.kindsMissing.replace("{}", missing.map(kind => Styles.kindLabel(kind)).join(", "))
                color: Theme.red
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize - 1
            }
        }

        Repeater {
            model: item.open ? item.folders : []

            DefaultFolder {
                required property var modelData
                folder: modelData
                current: item.kinds[modelData.kind] === modelData.name
                enabled: !item.busy
                onAssignRequested: (kind, sync, makeDefault) => item.accounts.assignFolder(item.name, folder.name, kind, sync, makeDefault)
            }
        }

        Item {
            width: parent.width
            height: 1

            Rule {
                inset: 24
            }
        }

        SettingRow {
            title: Strings.removeAccount
            hint: Strings.removeHint

            Row {
                spacing: 8

                Pill {
                    visible: !item.confirming
                    text: Strings.remove
                    onClicked: item.confirming = true
                }

                Pill {
                    visible: item.confirming
                    text: Strings.purge
                    on: item.purge
                    onClicked: item.purge = !item.purge
                }

                Pill {
                    visible: item.confirming
                    text: Strings.remove
                    on: true
                    outlined: true
                    onClicked: item.accounts.remove(item.name, item.purge)
                }

                Pill {
                    visible: item.confirming
                    text: Strings.cancel
                    onClicked: item.confirming = false
                }
            }
        }
    }

    Rule {
        visible: !item.open
        inset: 24
    }
}
