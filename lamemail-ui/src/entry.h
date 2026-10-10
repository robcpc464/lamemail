#pragma once

#include <QByteArray>
#include <QDebug>
#include <QJsonValue>
#include <QList>
#include <QString>
#include <QStringList>
#include <QVariantMap>
#include <optional>

struct Addr {
    QString name;
    QString address;
};

struct Style {
    QString icon;
    bool dense = false;
    bool dim = false;
    bool strike = false;
    bool italic = false;
    int indent = 0;
    QString color;

    bool operator==(const Style&) const = default;
};

struct Flags {
    bool seen = false;
    bool answered = false;
    bool forwarded = false;
    bool flagged = false;

    bool operator==(const Flags&) const = default;
};

struct Entry {
    QString account;
    QString color;
    Style style;
    QString kind;
    QString path;
    QString received;
    QString messageId;
    QList<Addr> from;
    QList<Addr> to;
    QList<Addr> cc;
    QString subject;
    Flags flags;
    bool pinned = false;
    int attachments = 0;
};

struct Overrides {
    std::optional<QString> icon;
    std::optional<bool> dense;
    std::optional<bool> dim;
    std::optional<bool> strike;
    std::optional<bool> italic;
    std::optional<int> indent;
    std::optional<QString> color;

    bool isEmpty() const;
};

struct Folder {
    QString kind;
    Style style;
    Overrides set;
    QStringList accounts;
    int messages = 0;
    int unread = 0;
};

struct Account {
    QString id;
    QString host;
    int port = 0;
    QString username;
    QString smtpHost;
    int smtpPort = 0;
    bool copyToSent = false;
    QString color;
    QString signature;
    QString contextFolder;
    bool archiveByYear = false;
    QString from;
    QString password;
    QVariantMap kinds;
    QVariantList folders;

    bool operator==(const Account&) const = default;
};

struct Contact {
    QString name;
    QString address;
    QString account;
    QString color;
    QString last;
    bool written = false;

    bool operator==(const Contact&) const = default;
};

struct AiProgram {
    QString program;
    QString name;
    QString url;
    bool found = false;
    bool running = false;
    QString command;
    QString ollama;

    bool operator==(const AiProgram&) const = default;
};

struct AiSetup {
    QString active;
    QString command;
    QString ollama;
    QString custom;
    QList<AiProgram> programs;
    int contextLimit = 0;
    QVariantList contextLimits;

    bool operator==(const AiSetup&) const = default;
};

struct ContextEntry {
    QString name;
    QString state;
};

struct UsedMail {
    QString path;
    QString subject;
    QString date;
};

struct Suggestion {
    QString path;
    QString text;
    QString question;
    std::optional<QList<ContextEntry>> context;
    QStringList searches;
    QList<UsedMail> found;
    QList<UsedMail> earlier;
};

struct SyncResult {
    QString account;
    int added = 0;
    int removed = 0;
    int updated = 0;
    QString error;
};

struct Arrival {
    QString path;
    QList<Addr> from;
    QString subject;
};

struct Moved {
    QString path;
    QString from;
    QString folder;
    Style style;
    QString kind;
};

struct Dropped {
    QString messageId;
    QString reason;
};

struct SentMail {
    QString messageId;
    QList<Addr> people;
    QString error;
};

struct PushResult {
    QString account;
    QList<Dropped> dropped;
    QList<SentMail> sent;
    QString error;
};

struct SyncProgress {
    QString account;
    QString folder;
    int fetched = 0;
    int of = 0;
    int folderNumber = 0;
    int folders = 0;

    QVariantMap toMap() const;
};

struct WatchLine {
    QString account;
    QList<Arrival> arrived;
    QString error;
    bool retrying = false;
    bool syncing = false;
    std::optional<SyncProgress> progress;
};

QList<Addr> addrs(const QJsonValue& value);
std::optional<Entry> entryFromJson(const QByteArray& line, QString* error = nullptr);
std::optional<Folder> folderFromJson(const QByteArray& line, QString* error = nullptr);
std::optional<Account> accountFromJson(const QByteArray& line, QString* error = nullptr);
std::optional<SyncResult> syncResultFromJson(const QByteArray& line, QString* error = nullptr);
std::optional<WatchLine> watchLineFromJson(const QByteArray& line, QString* error = nullptr);
std::optional<SyncProgress> progressFromJson(const QByteArray& line);
std::optional<PushResult> pushResultFromJson(const QByteArray& line, QString* error = nullptr);
std::optional<Moved> movedFromJson(const QByteArray& line, QString* error = nullptr);
std::optional<Contact> contactFromJson(const QByteArray& line, QString* error = nullptr);
std::optional<AiSetup> aiSetupFromJson(const QByteArray& line, QString* error = nullptr);
std::optional<Suggestion> suggestionFromJson(const QByteArray& line, QString* error = nullptr);

template <class T>
QList<T> fromJsonLines(
    const QByteArray& out, std::optional<T> (*parse)(const QByteArray&, QString*))
{
    QList<T> items;
    for (const QByteArray& line : out.split('\n')) {
        if (line.trimmed().isEmpty())
            continue;
        QString error;
        if (auto item = parse(line, &error))
            items.append(std::move(*item));
        else
            qWarning().noquote() << "lamemail-ui:" << error;
    }
    return items;
}
