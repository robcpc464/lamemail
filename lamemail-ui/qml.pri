CONFIG += qmltypes
QML_IMPORT_NAME = Lamemail
QML_IMPORT_MAJOR_VERSION = 1
for(module, $$list($$resolve_depends(QT, "QT.", ".depends" ".run_depends"))): \
    QML_FOREIGN_METATYPES += \
        $$files($$[QT_INSTALL_ARCHDATA]/metatypes/$$lower($$eval(QT.$${module}.module))_metatypes.json)
qml.files = $$files($$PWD/qml/*.qml)
qml.base = $$PWD
qml.prefix = /
RESOURCES += qml
