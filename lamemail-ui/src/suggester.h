#pragma once

#include <QObject>
#include <QProcess>
#include <QString>
#include <QStringList>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>

class Suggester : public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(bool running READ running NOTIFY runningChanged)

public:
    using QObject::QObject;
    ~Suggester() override;

    bool running() const { return m_process != nullptr; }
    Q_INVOKABLE void suggest(const QString& path, const QString& instruction, const QStringList& to,
        const QStringList& cc, const QString& before = { }, const QString& after = { },
        bool written = false);
    Q_INVOKABLE void cancel();

signals:
    void runningChanged();
    void suggested(const QString& text);
    void asked(const QString& question);
    void reported(const QVariantMap& report);
    void failed(const QString& error);

private:
    void finish(int code, QProcess::ExitStatus status);
    QProcess* release();

    QProcess* m_process = nullptr;
};
