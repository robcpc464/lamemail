CONFIG += qmltypes
QML_IMPORT_NAME = Lamemail
QML_IMPORT_MAJOR_VERSION = 1
qml.files = $$files($$PWD/qml/*.qml)
qml.base = $$PWD
qml.prefix = /
RESOURCES += qml
