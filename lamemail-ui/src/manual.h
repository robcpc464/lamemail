#pragma once

#include <QObject>
#include <QString>
#include <QVariantList>
#include <QtQml/qqmlregistration.h>

class Manual : public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QVariantList commands READ commands NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY changed)

public:
    using QObject::QObject;

    QVariantList commands() const { return m_commands; }
    QString error() const { return m_error; }

    Q_INVOKABLE void load();

signals:
    void changed();

private:
    QVariantList m_commands;
    QString m_error;
};
