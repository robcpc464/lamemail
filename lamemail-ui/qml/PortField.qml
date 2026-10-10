import QtQuick
import Lamemail

Field {
    implicitWidth: 92
    horizontalAlignment: TextInput.AlignHCenter
    placeholderText: Strings.defaultPort
    inputMethodHints: Qt.ImhDigitsOnly
    validator: IntValidator {
        bottom: 1
        top: 65535
    }
}
