#include "sender.h"

#include <QJsonDocument>
#include <QJsonObject>
#include <QStringList>

void Sender::send(const QVariantMap& job) { note(QStringLiteral("send"), job, &Sender::finished); }

void Sender::save(const QVariantMap& job) { note(QStringLiteral("draft"), job, &Sender::saved); }

void Sender::discard(const QVariantMap& job)
{
    run({ QStringLiteral("discard"), QStringLiteral("--"),
            job.value(QStringLiteral("account")).toString(),
            job.value(QStringLiteral("draftId")).toString() },
        { }, [this, job](const lamemail::Outcome& outcome) {
            emit discarded(job, outcome.ok ? QString() : outcome.error);
        });
}

void Sender::deleteForGood(const QString& path)
{
    run({ QStringLiteral("delete"), QStringLiteral("--"), path }, { },
        [this, path](const lamemail::Outcome& outcome) {
            emit deletedForGood(path, outcome.ok ? QString() : outcome.error);
        });
}

void Sender::run(const QStringList& command, const QByteArray& input,
    const std::function<void(const lamemail::Outcome&)>& done)
{
    ++m_noting;
    emit notingChanged();
    lamemail::run(
        this, command,
        [this, done](const lamemail::Outcome& outcome) {
            --m_noting;
            emit notingChanged();
            done(outcome);
        },
        { }, input);
}

void Sender::note(const QString& verb, const QVariantMap& job,
    void (Sender::*done)(const QVariantMap&, const QString&, const QString&))
{
    const auto value = [&](const char* key) { return job.value(QLatin1String(key)); };
    const QString path = value("path").toString();
    QStringList command { verb, QStringLiteral("--json"),
        QStringLiteral("--subject=") + value("subject").toString() };
    if (!path.isEmpty())
        command.append(
            (value("forward").toBool() ? QStringLiteral("--forward=") : QStringLiteral("--reply="))
            + path);
    const QString from = value("from").toString();
    if (!from.isEmpty())
        command.append(QStringLiteral("--from=") + from);
    for (const char* kind : { "to", "cc", "bcc" }) {
        for (const QString& address : value(kind).toStringList())
            command.append(QStringLiteral("--%1=%2").arg(QLatin1String(kind), address));
    }
    for (const QString& reference : value("references").toStringList())
        command.append(QStringLiteral("--attachment=") + reference);
    for (const QString& file : value("files").toStringList())
        command.append(QStringLiteral("--file=") + file);
    const QString draftId = value("draftId").toString();
    if (!draftId.isEmpty())
        command.append(QStringLiteral("--draft=") + draftId);
    if (path.isEmpty())
        command << QStringLiteral("--") << value("account").toString();
    run(command, value("text").toString().toUtf8(),
        [this, job, done](const lamemail::Outcome& outcome) {
            const QJsonObject noted = QJsonDocument::fromJson(outcome.out).object();
            emit(this->*done)(job, noted.value(QLatin1String("message_id")).toString(),
                outcome.ok ? QString() : outcome.error);
        });
}
