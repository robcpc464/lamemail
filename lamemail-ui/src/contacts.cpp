#include "contacts.h"

#include "lamemail.h"
#include "text.h"

#include <QDateTime>
#include <algorithm>

namespace {

bool matches(const Contact& contact, const QStringList& terms)
{
    for (const QString& term : terms) {
        if (!contact.name.contains(term, Qt::CaseInsensitive)
            && !contact.address.contains(term, Qt::CaseInsensitive))
            return false;
    }
    return true;
}

}

int Contacts::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : int(m_rows.size());
}

QVariant Contacts::data(const QModelIndex& index, int role) const
{
    if (!index.isValid() || index.row() >= m_rows.size())
        return { };
    const Contact& row = m_rows.at(index.row());
    switch (role) {
    case NameRole:
        return row.name;
    case AddressRole:
        return row.address;
    case AccountRole:
        return row.account;
    case AccountColorRole:
        return row.color;
    case TimeRole: {
        const auto when = parseTime(row.last);
        return when ? shortTime(*when, QDateTime::currentDateTime(), lang()) : row.last;
    }
    case WrittenRole:
        return row.written;
    default:
        return { };
    }
}

QHash<int, QByteArray> Contacts::roleNames() const
{
    return {
        { NameRole, "name" },
        { AddressRole, "address" },
        { AccountRole, "account" },
        { AccountColorRole, "accountColor" },
        { TimeRole, "time" },
        { WrittenRole, "written" },
    };
}

void Contacts::setQuery(const QString& query)
{
    if (m_query == query)
        return;
    m_query = query;
    emit queryChanged();
    arrange();
}

void Contacts::setChosen(const QStringList& chosen)
{
    if (m_chosen == chosen)
        return;
    m_chosen = chosen;
    emit chosenChanged();
}

void Contacts::reload()
{
    if (m_loading) {
        m_pending = true;
        return;
    }
    m_pending = false;
    setLoading(true);
    lamemail::run(this, { QStringLiteral("contacts"), QStringLiteral("--json") },
        [this](const lamemail::Outcome& outcome) {
            if (outcome.ok) {
                QList<Contact> all = fromJsonLines(outcome.out, contactFromJson);
                if (all != m_all) {
                    m_all = std::move(all);
                    arrange();
                }
                setError({ });
            } else {
                setError(outcome.error);
            }
            setLoading(false);
            if (m_pending)
                reload();
        });
}

QString Contacts::accountOf(const QString& address) const
{
    const QString wanted = address.trimmed();
    for (const Contact& contact : m_all) {
        if (contact.address.compare(wanted, Qt::CaseInsensitive) == 0)
            return contact.account;
    }
    return { };
}

void Contacts::arrange()
{
    const QStringList terms = m_query.split(QLatin1Char(' '), Qt::SkipEmptyParts);
    QHash<QString, qsizetype> ranks;
    for (const QString& address : std::as_const(m_chosen)) {
        const QString key = address.toLower();
        if (!ranks.contains(key))
            ranks.insert(key, ranks.size());
    }
    QList<Contact> chosen;
    QList<Contact> others;
    for (const Contact& contact : std::as_const(m_all)) {
        if (matches(contact, terms))
            (ranks.contains(contact.address.toLower()) ? chosen : others).append(contact);
    }
    std::stable_sort(chosen.begin(), chosen.end(), [&ranks](const Contact& a, const Contact& b) {
        return ranks.value(a.address.toLower()) < ranks.value(b.address.toLower());
    });
    beginResetModel();
    m_rows = chosen + others;
    endResetModel();
}

void Contacts::setLoading(bool loading)
{
    if (m_loading == loading)
        return;
    m_loading = loading;
    emit loadingChanged();
}

void Contacts::setError(const QString& error)
{
    if (m_error == error)
        return;
    m_error = error;
    emit errorChanged();
}
