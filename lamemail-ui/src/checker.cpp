#include "checker.h"

#include "entry.h"
#include "lamemail.h"

#include <utility>

QStringList Checker::pending() const
{
    QStringList paths { m_running, m_waiting };
    paths.removeAll(QString());
    return paths;
}

void Checker::check(const QString& path)
{
    if (path == m_running || path == m_waiting)
        return;
    if (m_running.isEmpty()) {
        run(path);
        return;
    }
    if (!m_waiting.isEmpty())
        emit dropped(m_waiting);
    m_waiting = path;
    emit pendingChanged();
}

void Checker::run(const QString& path)
{
    m_running = path;
    emit pendingChanged();
    lamemail::run(this,
        { QStringLiteral("ai"), QStringLiteral("check"), QStringLiteral("--json"),
            QStringLiteral("--"), path },
        [this, path](const lamemail::Outcome& outcome) {
            m_running.clear();
            if (m_waiting.isEmpty())
                emit pendingChanged();
            else
                run(std::exchange(m_waiting, { }));
            const auto verdict = outcome.ok ? checkedFromJson(outcome.out.trimmed()) : std::nullopt;
            if (verdict)
                emit checked(path, verdict->suspicious, verdict->kind, verdict->reason);
            else
                emit dropped(path);
        });
}
