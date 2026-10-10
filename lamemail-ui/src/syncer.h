#pragma once

#include "entry.h"
#include "lamemail.h"

#include <QList>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>
#include <optional>

class Syncer : public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(bool running READ running NOTIFY runningChanged)
    Q_PROPERTY(QVariantMap progress READ progress NOTIFY progressChanged)

public:
    using QObject::QObject;

    bool running() const { return m_running; }
    QVariantMap progress() const { return m_progress; }
    Q_INVOKABLE void sync(const QStringList& accounts = { });

signals:
    void runningChanged();
    void progressChanged();
    void finished(bool ok, const QVariantMap& counts, const QString& error);

private:
    void read(const QByteArray& out);
    void finish(const lamemail::Outcome& outcome);
    void setProgress(const QVariantMap& progress);

    bool m_running = false;
    std::optional<QStringList> m_queued;
    QByteArray m_pending;
    QList<SyncResult> m_results;
    QVariantMap m_progress;
};
