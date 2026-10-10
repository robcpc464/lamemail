#include "feed.h"

#include "lamemail.h"
#include "text.h"

#include <QDebug>
#include <QSet>
#include <algorithm>
#include <iterator>

namespace {

constexpr qint64 freshMs = 2000;

bool differs(const Entry& a, const Entry& b)
{
    return a.flags != b.flags || a.pinned != b.pinned || a.style != b.style || a.kind != b.kind
        || a.color != b.color || a.subject != b.subject || a.attachments != b.attachments;
}

}

Feed::Feed(QObject* parent)
    : QAbstractListModel(parent)
{
    m_clock.start();
}

int Feed::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : int(m_rows.size());
}

QVariant Feed::data(const QModelIndex& index, int role) const
{
    if (!index.isValid() || size_t(index.row()) >= m_rows.size())
        return { };
    const Row& row = m_rows.at(index.row());
    const Entry& entry = row.entry;
    switch (role) {
    case AccountRole:
        return entry.account;
    case AccountColorRole:
        return entry.color;
    case IconRole:
        return entry.style.icon;
    case DenseRole:
        return entry.style.dense;
    case DimRole:
        return entry.style.dim;
    case StrikeRole:
        return entry.style.strike;
    case ItalicRole:
        return entry.style.italic;
    case IndentRole:
        return entry.style.indent;
    case StyleColorRole:
        return entry.style.color;
    case SenderRole:
        return row.sender;
    case RecipientRole:
        return row.recipient;
    case MoreRecipientsRole:
        return row.more;
    case OtherRecipientsRole:
        return row.others;
    case SubjectRole:
        return entry.subject;
    case TimeRole:
        return row.time;
    case SeenRole:
        return entry.flags.seen;
    case FlaggedRole:
        return entry.flags.flagged;
    case AnsweredRole:
        return entry.flags.answered;
    case ForwardedRole:
        return entry.flags.forwarded;
    case FreshRole:
        return fresh(row);
    case AttachmentsRole:
        return entry.attachments;
    case PathRole:
        return entry.path;
    case DayRole:
        return QString::fromUtf8(entry.pinned ? texts().starred : texts().days[row.day]);
    case KindRole:
        return entry.kind;
    default:
        return { };
    }
}

QHash<int, QByteArray> Feed::roleNames() const
{
    return {
        { AccountRole, "account" },
        { AccountColorRole, "accountColor" },
        { IconRole, "icon" },
        { DenseRole, "dense" },
        { DimRole, "dim" },
        { StrikeRole, "strike" },
        { ItalicRole, "italic" },
        { IndentRole, "indent" },
        { StyleColorRole, "styleColor" },
        { SenderRole, "sender" },
        { RecipientRole, "recipient" },
        { MoreRecipientsRole, "moreRecipients" },
        { OtherRecipientsRole, "otherRecipients" },
        { SubjectRole, "subject" },
        { TimeRole, "time" },
        { SeenRole, "seen" },
        { FlaggedRole, "flagged" },
        { AnsweredRole, "answered" },
        { ForwardedRole, "forwarded" },
        { FreshRole, "fresh" },
        { AttachmentsRole, "attachments" },
        { PathRole, "path" },
        { DayRole, "day" },
        { KindRole, "kind" },
    };
}

void Feed::reload()
{
    m_started = true;
    if (m_loading) {
        m_pending = true;
        return;
    }
    m_pending = false;
    m_incoming.clear();
    m_buffer.clear();
    setStreaming(m_rows.empty());
    setLoading(true);
    setError({ });
    const QStringList arguments = listArguments();
    lamemail::run(
        this, arguments,
        [this, arguments](const lamemail::Outcome& outcome) {
            read(outcome.out, true);
            const bool stale = !m_jobs.isEmpty();
            if (!outcome.ok) {
                m_incoming.clear();
                if (m_streaming && !m_rows.empty())
                    remove(0, qsizetype(m_rows.size()));
                setError(outcome.error);
            } else if (stale) {
                m_incoming.clear();
                m_reloadAfterJobs = true;
            } else {
                if (!m_streaming)
                    merge(std::exchange(m_incoming, { }), arguments == m_shown);
                m_shown = arguments;
            }
            setLoading(false);
            updateCounts();
            if (m_pending && !stale)
                reload();
        },
        [this](const QByteArray& chunk) { read(chunk, false); });
}

QStringList Feed::listArguments() const
{
    QStringList arguments { QStringLiteral("list"), QStringLiteral("--json") };
    if (!m_query.trimmed().isEmpty()) {
        arguments << QStringLiteral("--search") << m_query;
    } else {
        for (const QString& kind : m_kinds)
            arguments << QStringLiteral("--kind") << kind;
    }
    if (m_flaggedFirst)
        arguments << QStringLiteral("--flagged-first");
    return arguments;
}

void Feed::setQuery(const QString& query)
{
    if (m_query == query)
        return;
    m_query = query;
    emit queryChanged();
    reload();
}

void Feed::setFlaggedFirst(bool flaggedFirst)
{
    if (m_flaggedFirst == flaggedFirst)
        return;
    m_flaggedFirst = flaggedFirst;
    emit flaggedFirstChanged();
    if (m_started)
        reload();
}

void Feed::setKinds(const QStringList& kinds)
{
    if (m_kinds == kinds)
        return;
    m_kinds = kinds;
    emit kindsChanged();
    if (m_started)
        reload();
}

void Feed::markSeen(const QString& path, bool seen)
{
    mark(path, { .seen = seen, .flagged = { } },
        seen ? QStringLiteral("seen") : QStringLiteral("unseen"));
}

void Feed::markFlagged(const QString& path, bool flagged)
{
    mark(path, { .seen = { }, .flagged = flagged },
        flagged ? QStringLiteral("flagged") : QStringLiteral("unflagged"));
}

void Feed::Mark::applyTo(Flags& flags) const
{
    flags.seen = seen.value_or(flags.seen);
    flags.flagged = flagged.value_or(flags.flagged);
}

void Feed::mark(const QString& path, const Mark& change, const QString& argument)
{
    const qsizetype at = indexOf(path, 0);
    if (at < qsizetype(m_rows.size())) {
        Entry& entry = m_rows[at].entry;
        Flags flags = entry.flags;
        change.applyTo(flags);
        if (flags == entry.flags)
            return;
        entry.flags = flags;
        emit dataChanged(index(int(at)), index(int(at)), { SeenRole, FlaggedRole });
        updateCounts();
    }
    enqueue({ path, { QStringLiteral("flag"), path, argument } });
}

void Feed::move(const QString& path, const QString& folder)
{
    enqueue({ path, { QStringLiteral("move"), QStringLiteral("--json"), path, folder } });
}

QString Feed::subject(const QString& path) const
{
    const qsizetype at = indexOf(path, 0);
    return at < qsizetype(m_rows.size()) ? m_rows[at].entry.subject : QString();
}

void Feed::enqueue(const Job& job)
{
    m_jobs.append(job);
    if (m_jobs.size() == 1)
        runJob();
}

void Feed::runJob()
{
    const Job job = m_jobs.first();
    lamemail::run(this, job.command, [this, job](const lamemail::Outcome& outcome) {
        m_jobs.removeFirst();
        if (!outcome.ok) {
            m_jobs.removeIf([&](const Job& other) { return other.path == job.path; });
            m_reloadAfterJobs = true;
            emit actionFailed(job.path, outcome.error);
        } else if (job.command.constFirst() == QLatin1String("move")) {
            settleMove(job.command.constLast(), outcome.out);
        }
        if (!m_jobs.isEmpty())
            runJob();
        else if (std::exchange(m_reloadAfterJobs, false))
            reload();
    });
}

void Feed::settleMove(const QString& target, const QByteArray& out)
{
    m_reloadAfterJobs = true;
    QString error;
    const auto result = movedFromJson(out.trimmed(), &error);
    if (!result) {
        qWarning().noquote() << "lamemail-ui:" << error;
        return;
    }
    const qsizetype at = indexOf(result->path, 0);
    if (at < qsizetype(m_rows.size())) {
        m_rows[at].entry.style = result->style;
        m_rows[at].entry.kind = result->kind;
        emit dataChanged(index(int(at)), index(int(at)));
    }
    emit moved(result->path, result->from, result->folder, target);
}

void Feed::refreshTimes()
{
    if (m_rows.empty())
        return;
    const QDateTime now = QDateTime::currentDateTime();
    const bool newDay = now.date() != m_today;
    m_today = now.date();
    for (Row& row : m_rows) {
        if (row.when)
            row.time = shortTime(*row.when, now, lang());
        if (newDay)
            row.day = dayIndex(row.when, m_today);
    }
    emit dataChanged(index(0), index(int(m_rows.size()) - 1),
        newDay ? QList<int> { TimeRole, DayRole } : QList<int> { TimeRole });
}

void Feed::read(const QByteArray& chunk, bool final)
{
    m_buffer += chunk;
    if (final && !m_buffer.isEmpty() && !m_buffer.endsWith('\n'))
        m_buffer += '\n';
    const qsizetype complete = m_buffer.lastIndexOf('\n') + 1;
    const QList<Entry> batch = fromJsonLines(m_buffer.first(complete), entryFromJson);
    m_buffer.remove(0, complete);
    if (m_streaming)
        append(batch);
    else
        m_incoming += batch;
}

void Feed::append(const QList<Entry>& batch)
{
    if (batch.isEmpty())
        return;
    const QDateTime now = QDateTime::currentDateTime();
    std::vector<Row> rows;
    rows.reserve(batch.size());
    for (const Entry& entry : batch)
        rows.push_back(row(entry, now, false));
    insert(qsizetype(m_rows.size()), std::move(rows));
    updateCounts();
}

void Feed::merge(const QList<Entry>& incoming, bool fresh)
{
    QSet<QString> keep;
    for (const Entry& entry : incoming)
        keep.insert(entry.path);
    QSet<QString> shownPaths;
    QSet<QString> shownIds;
    for (const Row& row : m_rows) {
        shownPaths.insert(row.entry.path);
        if (!row.entry.messageId.isEmpty())
            shownIds.insert(row.entry.messageId);
    }
    const auto isNew = [&](const Entry& entry) {
        return fresh && !shownPaths.contains(entry.path) && !shownIds.contains(entry.messageId);
    };
    qsizetype end = qsizetype(m_rows.size());
    while (end > 0) {
        if (keep.contains(m_rows[end - 1].entry.path)) {
            --end;
            continue;
        }
        qsizetype start = end - 1;
        while (start > 0 && !keep.contains(m_rows[start - 1].entry.path))
            --start;
        remove(start, end);
        end = start;
    }

    QSet<QString> waiting;
    for (const Row& row : m_rows)
        waiting.insert(row.entry.path);
    const QDateTime now = QDateTime::currentDateTime();
    qsizetype position = 0;
    std::vector<Row> run;
    const auto flush = [&] {
        const qsizetype count = qsizetype(run.size());
        if (count == 0)
            return;
        insert(position, std::exchange(run, { }));
        position += count;
    };
    for (const Entry& entry : incoming) {
        if (!waiting.remove(entry.path)) {
            run.push_back(row(entry, now, isNew(entry)));
            continue;
        }
        flush();
        const qsizetype at = indexOf(entry.path, position);
        if (at == position + 1) {
            waiting.remove(m_rows[position].entry.path);
            remove(position, position + 1);
        } else if (at > position) {
            remove(at, at + 1);
            run.push_back(row(entry, now, false));
            continue;
        }
        if (differs(m_rows[position].entry, entry)) {
            m_rows[position] = row(entry, now, false);
            emit dataChanged(index(int(position)), index(int(position)));
        }
        ++position;
    }
    flush();
}

void Feed::insert(qsizetype at, std::vector<Row> rows)
{
    beginInsertRows({ }, int(at), int(at + qsizetype(rows.size()) - 1));
    m_rows.insert(m_rows.begin() + at, std::make_move_iterator(rows.begin()),
        std::make_move_iterator(rows.end()));
    endInsertRows();
}

void Feed::remove(qsizetype start, qsizetype end)
{
    beginRemoveRows({ }, int(start), int(end - 1));
    m_rows.erase(m_rows.begin() + start, m_rows.begin() + end);
    endRemoveRows();
}

qsizetype Feed::indexOf(const QString& path, qsizetype from) const
{
    const auto found = std::find_if(m_rows.begin() + from, m_rows.end(),
        [&](const Row& row) { return row.entry.path == path; });
    return qsizetype(found - m_rows.begin());
}

Feed::Row Feed::row(const Entry& entry, const QDateTime& now, bool fresh) const
{
    Row row;
    row.entry = entry;
    row.sender = entry.from.isEmpty() ? QString() : personMarkup(entry.from.first());
    QList<Addr> recipients = entry.to + entry.cc;
    if (!recipients.isEmpty())
        row.recipient = personMarkup(recipients.takeFirst());
    QStringList others;
    for (const Addr& addr : recipients)
        others.append(person(addr));
    row.more = int(others.size());
    row.others = others.join(QLatin1Char('\n'));
    row.when = parseTime(entry.received);
    row.time = row.when ? shortTime(*row.when, now, lang()) : entry.received;
    row.day = dayIndex(row.when, now.date());
    row.insertedAt = fresh ? m_clock.elapsed() : -1;
    return row;
}

bool Feed::fresh(const Row& row) const
{
    return row.insertedAt >= 0 && m_clock.elapsed() - row.insertedAt < freshMs;
}

void Feed::updateCounts()
{
    const int count = int(m_rows.size());
    const int unread = int(std::count_if(
        m_rows.begin(), m_rows.end(), [](const Row& row) { return !row.entry.flags.seen; }));
    if (m_count != count) {
        m_count = count;
        emit countChanged();
    }
    if (m_unread != unread) {
        m_unread = unread;
        emit unreadChanged();
    }
}

void Feed::setLoading(bool loading)
{
    if (m_loading == loading)
        return;
    m_loading = loading;
    emit loadingChanged();
}

void Feed::setStreaming(bool streaming)
{
    if (m_streaming == streaming)
        return;
    m_streaming = streaming;
    emit streamingChanged();
}

void Feed::setError(const QString& error)
{
    if (m_error == error)
        return;
    m_error = error;
    emit errorChanged();
}
