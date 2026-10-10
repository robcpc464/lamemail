#include "entry.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>

namespace {

Style style(const QJsonValue& value)
{
    const QJsonObject object = value.toObject();
    return {
        .icon = object.value(QLatin1String("icon")).toString(),
        .dense = object.value(QLatin1String("dense")).toBool(),
        .dim = object.value(QLatin1String("dim")).toBool(),
        .strike = object.value(QLatin1String("strike")).toBool(),
        .italic = object.value(QLatin1String("italic")).toBool(),
        .indent = object.value(QLatin1String("indent")).toInt(),
        .color = object.value(QLatin1String("color")).toString(),
    };
}

std::optional<bool> maybeBool(const QJsonObject& object, const char* key)
{
    const QJsonValue value = object.value(QLatin1String(key));
    if (!value.isBool())
        return std::nullopt;
    return value.toBool();
}

std::optional<int> maybeInt(const QJsonObject& object, const char* key)
{
    const QJsonValue value = object.value(QLatin1String(key));
    if (!value.isDouble())
        return std::nullopt;
    return value.toInt();
}

std::optional<QString> maybeString(const QJsonObject& object, const char* key)
{
    const QJsonValue value = object.value(QLatin1String(key));
    if (!value.isString())
        return std::nullopt;
    return value.toString();
}

QVariantList assignedFolders(const QJsonObject& folders)
{
    QVariantList list;
    for (auto folder = folders.begin(); folder != folders.end(); ++folder) {
        const QJsonObject assigned = folder.value().toObject();
        list.append(QVariantMap {
            { QStringLiteral("name"), folder.key() },
            { QStringLiteral("kind"), assigned.value(QLatin1String("kind")).toString() },
            { QStringLiteral("sync"), assigned.value(QLatin1String("sync")).toBool() },
        });
    }
    return list;
}

std::optional<QJsonObject> object(const QByteArray& line, const char* required, QString* error)
{
    QJsonParseError parse;
    const QJsonDocument document = QJsonDocument::fromJson(line, &parse);
    if (parse.error != QJsonParseError::NoError || !document.isObject()) {
        if (error)
            *error = parse.error != QJsonParseError::NoError ? parse.errorString()
                                                             : QStringLiteral("not a JSON object");
        return std::nullopt;
    }
    QJsonObject result = document.object();
    if (!result.value(QLatin1String(required)).isString()) {
        if (error)
            *error = QStringLiteral("missing field `%1`").arg(QLatin1String(required));
        return std::nullopt;
    }
    return result;
}

std::optional<SyncProgress> progressOf(const QJsonObject& json)
{
    if (!json.contains(QLatin1String("fetched")))
        return std::nullopt;
    return SyncProgress {
        .account = json.value(QLatin1String("account")).toString(),
        .folder = json.value(QLatin1String("folder")).toString(),
        .fetched = json.value(QLatin1String("fetched")).toInt(),
        .of = json.value(QLatin1String("of")).toInt(),
        .folderNumber = json.value(QLatin1String("folder_number")).toInt(),
        .folders = json.value(QLatin1String("folders")).toInt(),
    };
}

}

QList<Addr> addrs(const QJsonValue& value)
{
    QList<Addr> list;
    for (const QJsonValue& item : value.toArray()) {
        const QJsonObject addr = item.toObject();
        list.append({ addr.value(QLatin1String("name")).toString(),
            addr.value(QLatin1String("address")).toString() });
    }
    return list;
}

bool Overrides::isEmpty() const
{
    return !icon && !dense && !dim && !strike && !italic && !indent && !color;
}

std::optional<Entry> entryFromJson(const QByteArray& line, QString* error)
{
    const auto parsed = object(line, "path", error);
    if (!parsed)
        return std::nullopt;
    const QJsonObject& json = *parsed;
    const QJsonObject flags = json.value(QLatin1String("flags")).toObject();
    return Entry {
        .account = json.value(QLatin1String("account")).toString(),
        .color = json.value(QLatin1String("color")).toString(),
        .style = style(json.value(QLatin1String("style"))),
        .kind = json.value(QLatin1String("kind")).toString(),
        .path = json.value(QLatin1String("path")).toString(),
        .received = json.value(QLatin1String("received")).toString(),
        .messageId = json.value(QLatin1String("message_id")).toString(),
        .from = addrs(json.value(QLatin1String("from"))),
        .to = addrs(json.value(QLatin1String("to"))),
        .cc = addrs(json.value(QLatin1String("cc"))),
        .subject = json.value(QLatin1String("subject")).toString(),
        .flags = {
            .seen = flags.value(QLatin1String("seen")).toBool(),
            .answered = flags.value(QLatin1String("answered")).toBool(),
            .forwarded = json.value(QLatin1String("forwarded")).toBool(),
            .flagged = flags.value(QLatin1String("flagged")).toBool(),
        },
        .pinned = json.value(QLatin1String("pinned")).toBool(),
        .attachments = json.value(QLatin1String("attachments")).toInt(),
    };
}

std::optional<Folder> folderFromJson(const QByteArray& line, QString* error)
{
    const auto parsed = object(line, "kind", error);
    if (!parsed)
        return std::nullopt;
    const QJsonObject& json = *parsed;
    const QJsonObject set = json.value(QLatin1String("set")).toObject();
    const Overrides overrides {
        .icon = maybeString(set, "icon"),
        .dense = maybeBool(set, "dense"),
        .dim = maybeBool(set, "dim"),
        .strike = maybeBool(set, "strike"),
        .italic = maybeBool(set, "italic"),
        .indent = maybeInt(set, "indent"),
        .color = maybeString(set, "color"),
    };
    QStringList accounts;
    for (const QJsonValue& account : json.value(QLatin1String("accounts")).toArray())
        accounts.append(account.toString());
    return Folder {
        .kind = json.value(QLatin1String("kind")).toString(),
        .style = style(json.value(QLatin1String("style"))),
        .set = overrides,
        .accounts = accounts,
        .messages = json.value(QLatin1String("messages")).toInt(),
        .unread = json.value(QLatin1String("unread")).toInt(),
    };
}

std::optional<Account> accountFromJson(const QByteArray& line, QString* error)
{
    const auto parsed = object(line, "id", error);
    if (!parsed)
        return std::nullopt;
    const QJsonObject& json = *parsed;
    const QJsonObject smtp = json.value(QLatin1String("smtp")).toObject();
    return Account {
        .id = json.value(QLatin1String("id")).toString(),
        .host = json.value(QLatin1String("host")).toString(),
        .port = json.value(QLatin1String("port")).toInt(),
        .username = json.value(QLatin1String("username")).toString(),
        .smtpHost = smtp.value(QLatin1String("host")).toString(),
        .smtpPort = smtp.value(QLatin1String("port")).toInt(),
        .copyToSent = smtp.value(QLatin1String("copy_to_sent")).toBool(),
        .color = json.value(QLatin1String("color")).toString(),
        .signature = json.value(QLatin1String("signature")).toString(),
        .contextFolder = json.value(QLatin1String("context_folder")).toString(),
        .archiveByYear = json.value(QLatin1String("archive_by_year")).toBool(),
        .from
        = json.value(QLatin1String("from")).toObject().value(QLatin1String("address")).toString(),
        .password = json.value(QLatin1String("password")).toString(),
        .kinds = json.value(QLatin1String("kinds")).toObject().toVariantMap(),
        .folders = assignedFolders(json.value(QLatin1String("folders")).toObject()),
    };
}

std::optional<SyncResult> syncResultFromJson(const QByteArray& line, QString* error)
{
    const auto parsed = object(line, "account", error);
    if (!parsed)
        return std::nullopt;
    const QJsonObject& json = *parsed;
    return SyncResult {
        .account = json.value(QLatin1String("account")).toString(),
        .added = json.value(QLatin1String("new")).toInt(),
        .removed = json.value(QLatin1String("removed")).toInt(),
        .updated = json.value(QLatin1String("updated")).toInt(),
        .error = json.value(QLatin1String("error")).toString(),
    };
}

QVariantMap SyncProgress::toMap() const
{
    return {
        { QStringLiteral("account"), account },
        { QStringLiteral("folder"), folder },
        { QStringLiteral("fetched"), fetched },
        { QStringLiteral("of"), of },
        { QStringLiteral("folderNumber"), folderNumber },
        { QStringLiteral("folders"), folders },
    };
}

std::optional<SyncProgress> progressFromJson(const QByteArray& line)
{
    return progressOf(QJsonDocument::fromJson(line).object());
}

std::optional<WatchLine> watchLineFromJson(const QByteArray& line, QString* error)
{
    const auto parsed = object(line, "account", error);
    if (!parsed)
        return std::nullopt;
    const QJsonObject& json = *parsed;
    QList<Arrival> arrived;
    for (const QJsonValue& item : json.value(QLatin1String("arrived")).toArray()) {
        const QJsonObject mail = item.toObject();
        arrived.append({
            .path = mail.value(QLatin1String("path")).toString(),
            .from = addrs(mail.value(QLatin1String("from"))),
            .subject = mail.value(QLatin1String("subject")).toString(),
        });
    }
    return WatchLine {
        .account = json.value(QLatin1String("account")).toString(),
        .arrived = arrived,
        .error = json.value(QLatin1String("error")).toString(),
        .retrying = json.value(QLatin1String("retry")).isDouble(),
        .syncing = json.value(QLatin1String("syncing")).toBool(),
        .progress = progressOf(json),
    };
}

std::optional<PushResult> pushResultFromJson(const QByteArray& line, QString* error)
{
    const auto parsed = object(line, "account", error);
    if (!parsed)
        return std::nullopt;
    const QJsonObject& json = *parsed;
    QList<Dropped> dropped;
    for (const QJsonValue& item : json.value(QLatin1String("dropped")).toArray()) {
        const QJsonObject entry = item.toObject();
        dropped.append({
            .messageId = entry.value(QLatin1String("message_id")).toString(),
            .reason = entry.value(QLatin1String("reason")).toString(),
        });
    }
    QList<SentMail> sent;
    for (const QJsonValue& item : json.value(QLatin1String("sent")).toArray()) {
        const QJsonObject mail = item.toObject();
        sent.append({
            .messageId = mail.value(QLatin1String("message_id")).toString(),
            .people = addrs(mail.value(QLatin1String("to")))
                + addrs(mail.value(QLatin1String("cc"))) + addrs(mail.value(QLatin1String("bcc"))),
            .error = mail.value(QLatin1String("error")).toString(),
        });
    }
    return PushResult {
        .account = json.value(QLatin1String("account")).toString(),
        .dropped = dropped,
        .sent = sent,
        .error = json.value(QLatin1String("error")).toString(),
    };
}

std::optional<Moved> movedFromJson(const QByteArray& line, QString* error)
{
    const auto parsed = object(line, "path", error);
    if (!parsed)
        return std::nullopt;
    const QJsonObject& json = *parsed;
    return Moved {
        .path = json.value(QLatin1String("path")).toString(),
        .from = json.value(QLatin1String("from")).toString(),
        .folder = json.value(QLatin1String("folder")).toString(),
        .style = style(json.value(QLatin1String("style"))),
        .kind = json.value(QLatin1String("kind")).toString(),
    };
}

std::optional<Contact> contactFromJson(const QByteArray& line, QString* error)
{
    const auto parsed = object(line, "address", error);
    if (!parsed)
        return std::nullopt;
    const QJsonObject& json = *parsed;
    return Contact {
        .name = json.value(QLatin1String("name")).toString(),
        .address = json.value(QLatin1String("address")).toString(),
        .account = json.value(QLatin1String("account")).toString(),
        .color = json.value(QLatin1String("color")).toString(),
        .last = json.value(QLatin1String("last")).toString(),
        .written = json.value(QLatin1String("written")).toBool(),
    };
}

std::optional<AiSetup> aiSetupFromJson(const QByteArray& line, QString* error)
{
    QJsonParseError parse;
    const QJsonDocument document = QJsonDocument::fromJson(line, &parse);
    if (!document.isObject()) {
        if (error)
            *error = parse.error != QJsonParseError::NoError ? parse.errorString()
                                                             : QStringLiteral("not a JSON object");
        return std::nullopt;
    }
    const QJsonObject json = document.object();
    AiSetup setup {
        .active = json.value(QLatin1String("active")).toString(),
        .command = json.value(QLatin1String("command")).toString(),
        .ollama = json.value(QLatin1String("ollama")).toString(),
        .custom = json.value(QLatin1String("custom")).toString(),
        .programs = { },
        .contextLimit = json.value(QLatin1String("context_limit")).toInt(),
        .contextLimits = json.value(QLatin1String("context_limits")).toArray().toVariantList(),
    };
    for (const QJsonValue& value : json.value(QLatin1String("programs")).toArray()) {
        const QJsonObject program = value.toObject();
        setup.programs.append({
            .program = program.value(QLatin1String("program")).toString(),
            .name = program.value(QLatin1String("name")).toString(),
            .url = program.value(QLatin1String("url")).toString(),
            .found = program.value(QLatin1String("found")).toBool(),
            .running = program.value(QLatin1String("running")).toBool(),
            .command = program.value(QLatin1String("command")).toString(),
            .ollama = program.value(QLatin1String("ollama")).toString(),
        });
    }
    return setup;
}

std::optional<Suggestion> suggestionFromJson(const QByteArray& line, QString* error)
{
    const auto parsed = object(line, "path", error);
    if (!parsed)
        return std::nullopt;
    const QJsonValue question = parsed->value(QLatin1String("question"));
    if (!question.isString() && !parsed->value(QLatin1String("text")).isString()) {
        if (error)
            *error = QStringLiteral("missing field `text`");
        return std::nullopt;
    }
    std::optional<QList<ContextEntry>> context;
    if (const QJsonValue files = parsed->value(QLatin1String("context")); files.isArray()) {
        context.emplace();
        for (const QJsonValue file : files.toArray())
            context->append({ .name = file[QLatin1String("name")].toString(),
                .state = file[QLatin1String("state")].toString() });
    }
    const auto used = [&parsed](const char* key) {
        QList<UsedMail> mails;
        for (const QJsonValue mail : parsed->value(QLatin1String(key)).toArray())
            mails.append({ .path = mail[QLatin1String("path")].toString(),
                .subject = mail[QLatin1String("subject")].toString(),
                .date = mail[QLatin1String("date")].toString() });
        return mails;
    };
    QStringList searches;
    for (const QJsonValue search : parsed->value(QLatin1String("searches")).toArray())
        searches.append(search.toString());
    return Suggestion {
        .path = parsed->value(QLatin1String("path")).toString(),
        .text = parsed->value(QLatin1String("text")).toString(),
        .question = question.toString(),
        .context = context,
        .searches = searches,
        .found = used("found"),
        .earlier = used("earlier"),
    };
}
