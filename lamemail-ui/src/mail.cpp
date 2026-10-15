#include "mail.h"

#include "entry.h"
#include "text.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>

namespace {

constexpr int allAttachments = -1;

QVariantList peopleOf(const QJsonValue& value, const QString& kind = { })
{
    QVariantList people;
    for (const Addr& addr : addrs(value)) {
        QVariantMap person {
            { QStringLiteral("name"), addr.name },
            { QStringLiteral("address"), addr.address },
        };
        if (!kind.isEmpty())
            person.insert(QStringLiteral("kind"), kind);
        people.append(person);
    }
    return people;
}

QVariantMap draftOf(const QJsonObject& object)
{
    QVariantList people;
    for (const char* kind : { "to", "cc", "bcc" })
        people += peopleOf(object.value(QLatin1String(kind)), QString::fromLatin1(kind));
    const QList<Addr> from = addrs(object.value(QLatin1String("from")));
    QString text = object.value(QLatin1String("body")).toString();
    while (text.endsWith(QLatin1Char('\n')))
        text.chop(1);
    return {
        { QStringLiteral("draftId"), object.value(QLatin1String("message_id")).toString() },
        { QStringLiteral("from"), from.isEmpty() ? QString() : from.first().address },
        { QStringLiteral("parent"), object.value(QLatin1String("parent")).toString() },
        { QStringLiteral("people"), people },
        { QStringLiteral("subject"), object.value(QLatin1String("subject")).toString() },
        { QStringLiteral("text"), text },
    };
}

QVariantList withLinks(const QJsonArray& parts)
{
    QVariantList linked;
    for (const QJsonValue& value : parts) {
        const QJsonObject part = value.toObject();
        QVariantMap map = part.toVariantMap();
        if (part.value(QLatin1String("kind")).toString() == QLatin1String("quote"))
            map.insert(
                QStringLiteral("parts"), withLinks(part.value(QLatin1String("parts")).toArray()));
        else {
            const LinkedText text = linkedText(part.value(QLatin1String("text")).toString());
            map.insert(QStringLiteral("html"), text.html);
            map.insert(QStringLiteral("shortened"), text.shortened);
        }
        linked.append(map);
    }
    return linked;
}

}

void Mail::open(const QString& path)
{
    if (path != m_path) {
        m_path = path;
        setFlagged(false);
        m_shown = { };
        m_error.clear();
        emit changed();
        emit fetchingChanged();
    }
    reload();
}

void Mail::reload()
{
    if (m_path.isEmpty())
        return;
    const int generation = ++m_generation;
    setLoading(true);
    lamemail::run(this, { QStringLiteral("show"), QStringLiteral("--json"), m_path },
        [this, generation](const lamemail::Outcome& outcome) {
            if (generation != m_generation)
                return;
            QString error = outcome.error;
            Shown shown = m_shown;
            if (outcome.ok) {
                Shown parsed = parse(outcome.out, &error);
                if (error.isEmpty())
                    shown = std::move(parsed);
            }
            show(std::move(shown), error);
            setLoading(false);
        });
}

void Mail::openAttachment(int index)
{
    fetch(index, { }, [this](const lamemail::Outcome& outcome) {
        const QJsonObject saved = QJsonDocument::fromJson(outcome.out).object();
        emit attachmentReady(QUrl::fromLocalFile(saved.value(QLatin1String("path")).toString()));
    });
}

void Mail::downloadAttachments(const QString& folder)
{
    fetch(allAttachments, { QStringLiteral("--to=") + folder },
        [this, folder](const lamemail::Outcome& outcome) {
            emit attachmentsDownloaded(outcome.out.count('\n'), tildePath(folder));
        });
}

void Mail::fetch(int index, const QStringList& options,
    const std::function<void(const lamemail::Outcome&)>& done)
{
    const std::pair key { m_path, index };
    if (m_path.isEmpty() || m_fetching.contains(key))
        return;
    m_fetching.append(key);
    emit fetchingChanged();
    QStringList command { QStringLiteral("attachment"), QStringLiteral("--json") };
    command << options << QStringLiteral("--") << m_path;
    if (index != allAttachments)
        command << QString::number(index);
    lamemail::run(this, command, [this, key, done](const lamemail::Outcome& outcome) {
        m_fetching.removeOne(key);
        emit fetchingChanged();
        if (outcome.ok)
            done(outcome);
        else
            emit attachmentFailed(outcome.error);
    });
}

QVariantList Mail::opening() const
{
    QVariantList indexes;
    for (const auto& [path, index] : m_fetching) {
        if (path == m_path && index != allAttachments)
            indexes.append(index);
    }
    return indexes;
}

bool Mail::downloading() const { return m_fetching.contains(std::pair { m_path, allAttachments }); }

void Mail::setFlagged(bool flagged)
{
    if (m_shown.flagged == flagged)
        return;
    m_shown.flagged = flagged;
    emit flaggedChanged();
}

void Mail::show(Shown shown, const QString& error)
{
    setFlagged(shown.flagged);
    if (shown == m_shown && error == m_error)
        return;
    m_shown = std::move(shown);
    m_error = error;
    emit changed();
}

Mail::Shown Mail::parse(const QByteArray& json, QString* error)
{
    QJsonParseError parseError;
    const QJsonObject object = QJsonDocument::fromJson(json, &parseError).object();
    if (parseError.error != QJsonParseError::NoError) {
        *error = parseError.errorString();
        return { };
    }
    const auto field = [&](const char* name) { return object.value(QLatin1String(name)); };
    const QList<Addr> from = addrs(field("from"));
    const auto when = parseTime(field("received").toString());
    QVariantList attachments;
    for (const QJsonValue& value : field("attachments").toArray()) {
        const QJsonObject attachment = value.toObject();
        const QString name = attachment.value(QLatin1String("name")).toString();
        attachments.append(QVariantMap {
            { QStringLiteral("index"), attachment.value(QLatin1String("index")).toInt() },
            { QStringLiteral("name"),
                name.isEmpty() ? attachment.value(QLatin1String("type")).toString() : name },
            { QStringLiteral("size"),
                fileSize(attachment.value(QLatin1String("size")).toInteger()) },
            { QStringLiteral("inline"), attachment.value(QLatin1String("inline")).toBool() },
        });
    }
    return {
        .subject = field("subject").toString(),
        .sender = from.isEmpty() ? QString() : person(from.first()),
        .recipients = people(addrs(field("to")) + addrs(field("cc")), person),
        .time = when ? detailTime(*when, QDateTime::currentDateTime(), lang())
                     : field("received").toString(),
        .account = field("account").toString(),
        .color = field("color").toString(),
        .kind = field("kind").toString(),
        .answerFrom = field("answer_from").toObject().value(QLatin1String("address")).toString(),
        .answerTo = peopleOf(field("answer_to")),
        .answerCc = peopleOf(field("answer_cc")),
        .answerSubject = field("answer_subject").toString(),
        .forwardSubject = field("forward_subject").toString(),
        .quote = field("quote").toString(),
        .signature = field("signature").toString(),
        .draft = draftOf(object),
        .parts = withLinks(field("parts").toArray()),
        .attachments = attachments,
        .flagged = field("flags").toObject().value(QLatin1String("flagged")).toBool(),
    };
}

void Mail::setLoading(bool loading)
{
    if (m_loading == loading)
        return;
    m_loading = loading;
    emit loadingChanged();
}
