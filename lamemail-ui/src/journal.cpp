#include "journal.h"

#include "entry.h"
#include "text.h"
#include "texts.h"

#include <QDebug>
#include <QJsonDocument>
#include <QJsonObject>
#include <QStringList>
#include <utility>

namespace {

constexpr int pushDelayMs = 10000;

}

Journal::Journal(QObject* parent)
    : QObject(parent)
{
    m_soon.setSingleShot(true);
    m_soon.setInterval(pushDelayMs);
    connect(&m_soon, &QTimer::timeout, this, &Journal::push);
}

void Journal::reload()
{
    if (m_loading) {
        m_again = true;
        return;
    }
    m_loading = true;
    lamemail::run(this, { QStringLiteral("journal"), QStringLiteral("--json") },
        [this](const lamemail::Outcome& outcome) {
            m_loading = false;
            if (!outcome.ok) {
                qWarning().noquote() << "lamemail-ui: lamemail journal:" << outcome.error;
            } else {
                int count = 0;
                int sending = 0;
                QStringList leaving;
                QStringList answering;
                for (const QByteArray& line : outcome.out.split('\n')) {
                    if (line.trimmed().isEmpty())
                        continue;
                    const QJsonObject entry = QJsonDocument::fromJson(line).object();
                    const QString action = entry.value(QLatin1String("action")).toString();
                    const QString path = entry.value(QLatin1String("path")).toString();
                    if (action == QLatin1String("send")) {
                        ++sending;
                        if (!path.isEmpty())
                            answering.append(path);
                    } else {
                        ++count;
                        if (action == QLatin1String("move"))
                            leaving.append(path);
                    }
                }
                const bool written = std::exchange(m_entries, outcome.out) != outcome.out;
                if (written && count + sending > 0)
                    m_soon.start();
                if (sending == 0)
                    setFailure({ });
                if (count != m_count || sending != m_sending || leaving != m_leaving
                    || answering != m_answering) {
                    m_count = count;
                    m_sending = sending;
                    m_leaving = leaving;
                    m_answering = answering;
                    emit changed();
                }
            }
            if (std::exchange(m_again, false))
                reload();
        });
}

void Journal::push()
{
    if (m_pushing) {
        m_pushAgain = true;
        return;
    }
    m_soon.stop();
    setPushing(true);
    lamemail::run(this, { QStringLiteral("push"), QStringLiteral("--json") },
        [this](const lamemail::Outcome& outcome) {
            report(outcome);
            setPushing(false);
            reload();
            if (std::exchange(m_pushAgain, false))
                push();
        });
}

void Journal::report(const lamemail::Outcome& outcome)
{
    QStringList failures;
    if (!outcome.ok && outcome.out.isEmpty()) {
        qWarning().noquote() << "lamemail-ui: lamemail push:" << outcome.error;
        failures.append(outcome.error);
    }
    for (const PushResult& result : fromJsonLines(outcome.out, pushResultFromJson)) {
        if (!result.error.isEmpty()) {
            qWarning().noquote() << "lamemail-ui: lamemail push:" << result.account << result.error;
            failures.append(result.account + QStringLiteral(": ") + result.error);
        }
        for (const Dropped& entry : result.dropped)
            emit dropped(entry.reason, entry.messageId);
        for (const SentMail& mail : result.sent) {
            const QString names = people(mail.people, displayName);
            emit sent(mail.messageId,
                mail.error.isEmpty()
                    ? QString::fromUtf8(texts().sentTo).replace(QLatin1String("{}"), names)
                    : mail.error,
                mail.error.isEmpty());
        }
    }
    setFailure(failures.join(QLatin1Char('\n')));
}

void Journal::setPushing(bool pushing)
{
    if (m_pushing == pushing)
        return;
    m_pushing = pushing;
    emit pushingChanged();
}

void Journal::setFailure(const QString& failure)
{
    if (m_failure == failure)
        return;
    m_failure = failure;
    emit failureChanged();
}
