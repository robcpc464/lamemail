#pragma once

#include "entry.h"

#include <QObject>
#include <QString>
#include <QStringList>
#include <QVariantList>
#include <QtQml/qqmlregistration.h>

class Ai : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(QString active READ active NOTIFY changed)
    Q_PROPERTY(QString command READ command NOTIFY changed)
    Q_PROPERTY(QString ollama READ ollama NOTIFY changed)
    Q_PROPERTY(QString custom READ custom NOTIFY changed)
    Q_PROPERTY(QVariantList programs READ programs NOTIFY changed)
    Q_PROPERTY(bool ready READ ready NOTIFY changed)
    Q_PROPERTY(int contextLimit READ contextLimit NOTIFY changed)
    Q_PROPERTY(QVariantList contextLimits READ contextLimits NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)

public:
    using QObject::QObject;

    QString active() const { return m_setup.active; }
    QString command() const { return m_setup.command; }
    QString ollama() const { return m_setup.ollama; }
    QString custom() const { return m_setup.custom; }
    QVariantList programs() const;
    bool ready() const { return !m_setup.active.isEmpty(); }
    int contextLimit() const { return m_setup.contextLimit; }
    QVariantList contextLimits() const { return m_setup.contextLimits; }
    QString error() const { return m_error; }

    Q_INVOKABLE void reload();
    Q_INVOKABLE void choose(const QString& choice, const QString& value);
    Q_INVOKABLE void remove();
    Q_INVOKABLE void setContextLimit(int chars);

signals:
    void changed();
    void errorChanged();

private:
    void change(const QStringList& command);
    void setError(const QString& error);

    AiSetup m_setup;
    QString m_error;
};
