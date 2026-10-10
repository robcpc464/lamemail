pragma ComponentBehavior: Bound

import QtQuick
import Lamemail

Item {
    id: form

    required property Accounts accounts
    property bool open: false
    property bool busy: false
    property string failure
    readonly property bool complete: nameField.text.trim().length > 0 && hostField.text.trim().length > 0
                                     && userField.text.trim().length > 0 && passwordField.text.length > 0
                                     && [portField, smtpPortField].every((field) => field.text.length === 0
                                                                                    || field.acceptableInput)
    signal closed()

    width: parent ? parent.width : 0
    height: head.height + body.height

    onOpenChanged: {
        if (!open) {
            closed()
            return
        }
        for (const field of [nameField, hostField, portField, userField, passwordField, smtpHostField, smtpPortField])
            field.text = ""
        form.failure = ""
        nameField.forceActiveFocus()
    }

    Connections {
        target: form.accounts

        function onAddFinished(ok, name, message) {
            form.busy = false
            if (ok)
                form.open = false
            else
                form.failure = message
        }
    }

    Item {
        id: head
        width: parent.width
        height: 68

        Pill {
            anchors.left: parent.left
            anchors.leftMargin: 24
            anchors.verticalCenter: parent.verticalCenter
            icon: form.open ? "" : "plus"
            text: form.open ? Strings.cancel : Strings.addAccount
            on: form.open
            onClicked: form.open = !form.open
        }

        BusyLine {
            running: form.busy
        }
    }

    Fold {
        id: body
        anchors.top: head.bottom
        open: form.open

        SettingRow {
            title: Strings.accountName
            hint: Strings.accountNameHint

            Field {
                id: nameField
            }
        }

        SettingRow {
            title: Strings.imapServer

            Row {
                spacing: 8

                Field {
                    id: hostField
                }

                PortField {
                    id: portField
                }
            }
        }

        SettingRow {
            title: Strings.login

            Field {
                id: userField
            }
        }

        SettingRow {
            title: Strings.password
            hint: Strings.passwordHint

            Field {
                id: passwordField
                echoMode: TextInput.Password
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
            }
        }

        Item {
            width: parent.width
            height: 64

            Text {
                anchors.left: parent.left
                anchors.right: addButton.left
                anchors.leftMargin: 24
                anchors.rightMargin: 16
                anchors.verticalCenter: parent.verticalCenter
                text: form.failure
                color: Theme.red
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize - 1
                wrapMode: Text.Wrap
            }

            Pill {
                id: addButton
                anchors.right: parent.right
                anchors.rightMargin: 24
                anchors.verticalCenter: parent.verticalCenter
                text: Strings.add
                on: form.complete && !form.busy
                enabled: form.complete && !form.busy
                onClicked: {
                    form.busy = true
                    form.failure = ""
                    form.accounts.add(nameField.text.trim(), hostField.text.trim(), portField.text,
                                      userField.text.trim(), passwordField.text, smtpHostField.text.trim(),
                                      smtpPortField.text)
                }
            }
        }
    }
}
