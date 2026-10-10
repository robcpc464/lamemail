#include "syncer.h"

void Syncer::sync(const QStringList& accounts)
{
    if (m_running) {
        QStringList merged = accounts;
        if (m_queued) {
            const bool everything = accounts.isEmpty() || m_queued->isEmpty();
            merged = everything ? QStringList { } : *m_queued + accounts;
            merged.removeDuplicates();
        }
        m_queued = merged;
        return;
    }
    m_running = true;
    emit runningChanged();
    lamemail::run(
        this, QStringList { QStringLiteral("sync"), QStringLiteral("--json") } + accounts,
        [this](const lamemail::Outcome& outcome) { finish(outcome); },
        [this](const QByteArray& out) { read(out); });
}

void Syncer::read(const QByteArray& out)
{
    m_pending += out;
    for (qsizetype end = m_pending.indexOf('\n'); end >= 0; end = m_pending.indexOf('\n')) {
        const QByteArray line = m_pending.left(end);
        m_pending.remove(0, end + 1);
        if (line.trimmed().isEmpty())
            continue;
        if (const auto progress = progressFromJson(line))
            setProgress(progress->toMap());
        else
            m_results += fromJsonLines(line, syncResultFromJson);
    }
}

void Syncer::finish(const lamemail::Outcome& outcome)
{
    read(outcome.out + '\n');
    int synced = 0;
    int failed = 0;
    int added = 0;
    int removed = 0;
    int updated = 0;
    QString failure;
    for (const SyncResult& result : std::exchange(m_results, { })) {
        if (!result.error.isEmpty()) {
            if (failed++ == 0)
                failure = result.account + QStringLiteral(": ") + result.error;
            continue;
        }
        ++synced;
        added += result.added;
        removed += result.removed;
        updated += result.updated;
    }
    const QVariantMap counts { { QStringLiteral("accounts"), synced },
        { QStringLiteral("failed"), failed }, { QStringLiteral("added"), added },
        { QStringLiteral("removed"), removed }, { QStringLiteral("updated"), updated } };
    const QString error = failure.isEmpty() ? outcome.error : failure;
    m_pending.clear();
    setProgress({ });
    m_running = false;
    emit runningChanged();
    emit finished(outcome.ok, counts, error);
    if (const auto queued = std::exchange(m_queued, std::nullopt))
        sync(*queued);
}

void Syncer::setProgress(const QVariantMap& progress)
{
    if (m_progress == progress)
        return;
    m_progress = progress;
    emit progressChanged();
}
