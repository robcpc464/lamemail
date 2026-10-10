#pragma once

#include <QByteArray>
#include <QMap>
#include <QObject>
#include <QProcess>
#include <QString>
#include <QStringList>
#include <QTimer>
#include <QVariantList>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>

class MailWatch : public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QStringList syncing READ syncing NOTIFY stateChanged)
    Q_PROPERTY(QStringList failing READ failing NOTIFY stateChanged)
    Q_PROPERTY(QString failures READ failures NOTIFY stateChanged)
    Q_PROPERTY(QVariantMap progress READ progress NOTIFY stateChanged)

public:
    explicit MailWatch(QObject* parent = nullptr);
    ~MailWatch() override;

    QStringList syncing() const { return m_syncing; }
    QStringList failing() const { return m_failures.keys(); }
    QString failures() const;
    QVariantMap progress() const { return m_progress; }

    Q_INVOKABLE void start();
    Q_INVOKABLE void restart();

signals:
    void stateChanged();
    void synced();
    void arrived(const QVariantList& mails);
    void stopped(const QString& message);

private:
    void read();
    void handle(const QByteArray& line);
    void finish(int code, QProcess::ExitStatus status);
    void stop();
    void settle();

    QProcess* m_process = nullptr;
    QByteArray m_pending;
    QTimer m_again;
    QStringList m_syncing;
    QMap<QString, QString> m_failures;
    QVariantMap m_progress;
};
