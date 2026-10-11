#include "accounts.h"
#include "ai.h"
#include "contacts.h"
#include "entry.h"
#include "feed.h"
#include "folders.h"
#include "folding.h"
#include "instance.h"
#include "journal.h"
#include "lamemail.h"
#include "localfiles.h"
#include "mail.h"
#include "mailto.h"
#include "mailwatch.h"
#include "manual.h"
#include "notifier.h"
#include "opener.h"
#include "preferences.h"
#include "sender.h"
#include "styles.h"
#include "suggester.h"
#include "syncer.h"
#include "text.h"
#include "texts.h"
#include "theme.h"
#include "tray.h"
#include "watcher.h"

#include <QAbstractItemModelTester>
#include <QDesktopServices>
#include <QDir>
#include <QElapsedTimer>
#include <QFile>
#include <QGuiApplication>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickItem>
#include <QRandomGenerator>
#include <QScopeGuard>
#include <QSettings>
#include <QSignalSpy>
#include <QStandardPaths>
#include <QTemporaryDir>
#include <QTextBlock>
#include <QTextDocument>
#include <QTimeZone>
#include <QtTest>

namespace {

QDateTime at(const char* iso)
{
    return QDateTime::fromString(QString::fromLatin1(iso), Qt::ISODate);
}

QStringList anchors(const QTextDocument& document)
{
    QStringList found;
    for (QTextBlock block = document.begin(); block.isValid(); block = block.next()) {
        for (auto fragments = block.begin(); !fragments.atEnd(); ++fragments) {
            const QTextFragment fragment = fragments.fragment();
            if (fragment.charFormat().isAnchor())
                found.append(
                    fragment.charFormat().anchorHref() + QLatin1String(" = ") + fragment.text());
        }
    }
    return found;
}

QByteArray mail(const QString& id, bool seen = true, const char* color = "blue")
{
    return QStringLiteral(
               R"({"account":"private","color":"%3","style":{"icon":"incoming","dense":false,"dim":false,"strike":false,"italic":false,"indent":0},"path":"private/inbox/2026-09-15_1432-%1.json","received":"2026-09-15T14:32:05+02:00","message_id":"%1@example.com","from":[{"name":"Jane Doe","address":"jane@example.com"}],"to":[],"cc":[],"subject":"%1","flags":{"seen":%2,"answered":false,"flagged":false},"attachments":0,"preview":""})")
               .arg(id, QLatin1String(seen ? "true" : "false"), QLatin1String(color))
               .toUtf8()
        + '\n';
}

QByteArray mails(const QStringList& ids)
{
    QByteArray out;
    for (const QString& id : ids)
        out += mail(id);
    return out;
}

QByteArray folder(const char* kind, bool denseSet)
{
    return QStringLiteral(
               R"({"kind":"%1","style":{"icon":"incoming","dense":%2,"dim":false,"strike":false,"italic":false,"indent":%4},"set":{%3},"accounts":["private"],"messages":2,"unread":1})")
               .arg(QLatin1String(kind), QLatin1String(denseSet ? "true" : "false"),
                   QLatin1String(denseSet ? R"("dense":true)" : ""),
                   QLatin1String(denseSet ? "3" : "0"))
               .toUtf8()
        + '\n';
}

QByteArray pinned(const QString& id)
{
    return mail(id).replace(R"("attachments")", R"("pinned":true,"attachments")");
}

QStringList subjects(const Feed& feed)
{
    QStringList list;
    for (int row = 0; row < feed.rowCount(); ++row)
        list.append(feed.data(feed.index(row), Feed::SubjectRole).toString());
    return list;
}

void write(const QString& path, const QByteArray& content)
{
    QFile file(path);
    QVERIFY(file.open(QIODevice::WriteOnly));
    file.write(content);
}

QByteArray read(const QString& path)
{
    QFile file(path);
    return file.open(QIODevice::ReadOnly) ? file.readAll() : QByteArray();
}

}

class UrlCatcher : public QObject {
    Q_OBJECT

public:
    QList<QUrl> urls;

public slots:
    void open(const QUrl& url) { urls.append(url); }
};

class TestLamemail : public QObject {
    Q_OBJECT

    QTemporaryDir m_fake;

    void answer(const QByteArray& out, int code = 0, const QByteArray& err = { })
    {
        write(m_fake.filePath(QStringLiteral("out")), out);
        write(m_fake.filePath(QStringLiteral("err")), err);
        write(m_fake.filePath(QStringLiteral("code")), QByteArray::number(code));
    }

    static auto ownSettings(const QTemporaryDir& home)
    {
        QSettings::setPath(
            QSettings::NativeFormat, QSettings::UserScope, home.filePath(QStringLiteral("config")));
        QCoreApplication::setOrganizationName(QStringLiteral("lamemail"));
        return qScopeGuard([] { QCoreApplication::setOrganizationName({ }); });
    }

    static void load(Feed& feed)
    {
        feed.reload();
        QVERIFY(QTest::qWaitFor([&] { return !feed.loading(); }));
    }

private slots:
    void initTestCase()
    {
        QVERIFY(m_fake.isValid());
        const QString script = m_fake.filePath(QStringLiteral("lamemail"));
        write(script, R"sh(#!/bin/sh
dir=$(dirname "$0")
printf '%s\n' "$@" >"$dir/args"
cat >"$dir/in.$$"
{ printf '%s\n' "$@"; printf 'stdin:'; cat "$dir/in.$$"; echo; } >"$dir/entry.$$"
cat "$dir/entry.$$" >>"$dir/log"
mv "$dir/in.$$" "$dir/in"
rm "$dir/entry.$$"
[ -f "$dir/sleep-$3" ] && sleep "$(cat "$dir/sleep-$3")"
cat "$dir/out"
cat "$dir/err" >&2
exit "$(cat "$dir/code")"
)sh");
        QFile::setPermissions(script, QFileDevice::ReadOwner | QFileDevice::ExeOwner);
        qputenv("PATH", m_fake.path().toUtf8() + ':' + qgetenv("PATH"));
        lamemail::setDir(m_fake.path());
        answer({ });
    }

    void parsesTheDocumentedLine()
    {
        const QByteArray line
            = R"({"account":"private","color":"blue","folder":"inbox","style":{"icon":"incoming","dense":false,"dim":false,"strike":false,"italic":false,"indent":3,"color":"blue"},"kind":"inbox","uid":4711,"path":"private/inbox/2026-09-15_1432-offer-for-project-4711.json","received":"2026-09-15T14:32:05+02:00","date":"2026-09-15T14:32:00+02:00","from":[{"name":"Jane Doe","address":"jane@example.com"}],"to":[{"name":null,"address":"me@example.com"}],"cc":[],"subject":"Offer for project","flags":{"seen":true,"answered":false,"flagged":false,"draft":false,"keywords":[]},"size":234567,"attachments":1,"preview":"Hi, please find the offer attached. Jane"})";
        const auto entry = entryFromJson(line);
        QVERIFY(entry);
        QCOMPARE(entry->color, QStringLiteral("blue"));
        QCOMPARE(entry->style.icon, QStringLiteral("incoming"));
        QCOMPARE(entry->style.indent, 3);
        QCOMPARE(entry->style.color, QStringLiteral("blue"));
        QCOMPARE(entry->kind, QStringLiteral("inbox"));
        QCOMPARE(entry->path,
            QStringLiteral("private/inbox/2026-09-15_1432-offer-for-project-4711.json"));
        QCOMPARE(entry->from.first().name, QStringLiteral("Jane Doe"));
        QCOMPARE(entry->to.first().address, QStringLiteral("me@example.com"));
        QVERIFY(entry->to.first().name.isEmpty());
        QVERIFY(entry->cc.isEmpty());
        QVERIFY(entry->flags.seen);
        QVERIFY(!entry->flags.forwarded);
        QVERIFY(!entry->pinned);
        QCOMPARE(entry->attachments, 1);
        QVERIFY(entryFromJson(QByteArray(line).replace(R"("size")", R"("forwarded":true,"size")"))
                ->flags.forwarded);
        QVERIFY(entryFromJson(QByteArray(line).replace(R"("size")", R"("pinned":true,"size")"))
                ->pinned);
    }

    void rejectsLinesThatAreNotEntries()
    {
        QString error;
        QVERIFY(!entryFromJson("not json", &error));
        QVERIFY(!error.isEmpty());
        QVERIFY(!entryFromJson(R"({"account":"private"})", &error));
        QCOMPARE(error, QStringLiteral("missing field `path`"));
    }

    void parsesAFoldersLine()
    {
        const auto folder = folderFromJson(
            R"({"kind":"junk","style":{"icon":"mail","dense":false,"dim":true,"strike":false,"italic":false,"color":"green"},"set":{"icon":"mail","dense":false,"indent":2,"color":"green"},"accounts":["business"],"messages":1,"unread":1})");
        QVERIFY(folder);
        QCOMPARE(folder->kind, QStringLiteral("junk"));
        QCOMPARE(folder->set.icon, std::optional<QString>(QStringLiteral("mail")));
        QCOMPARE(folder->set.dense, std::optional<bool>(false));
        QCOMPARE(folder->set.indent, std::optional<int>(2));
        QCOMPARE(folder->set.color, std::optional<QString>(QStringLiteral("green")));
        QCOMPARE(folder->style.color, QStringLiteral("green"));
        QVERIFY(!folder->set.dim);
        QVERIFY(!folder->set.isEmpty());
        QVERIFY(folder->style.dim);
        QCOMPARE(folder->accounts, QStringList { QStringLiteral("business") });
        QCOMPARE(folder->unread, 1);
    }

    void explicitDirWins()
    {
        QCOMPARE(lamemail::syncDir(QStringLiteral("/x")), QStringLiteral("/x"));
        qunsetenv("LAMEMAIL_DIR");
        QCOMPARE(lamemail::syncDir({ }), QDir::homePath() + QStringLiteral("/.lamemail"));
        qputenv("LAMEMAIL_DIR", "/from/env");
        QCOMPARE(lamemail::syncDir({ }), QStringLiteral("/from/env"));
        qunsetenv("LAMEMAIL_DIR");
    }

    void commandsPassTheDirFirst()
    {
        lamemail::setDir(QStringLiteral("/sync"));
        QCOMPARE(lamemail::arguments({ QStringLiteral("list"), QStringLiteral("--json") }),
            (QStringList { QStringLiteral("--dir"), QStringLiteral("/sync"), QStringLiteral("list"),
                QStringLiteral("--json") }));
    }

    void theLastStderrLineIsTheFailure()
    {
        QCOMPARE(lamemail::lastLine(QStringLiteral(
                     "lamemail: private: failed: login\nlamemail: sync incomplete, see above\n")),
            std::optional<QString>(QStringLiteral("sync incomplete, see above")));
        QVERIFY(!lamemail::lastLine(QStringLiteral("\n")));
    }

    void personCombinesNameAndAddress()
    {
        QCOMPARE(person({ QStringLiteral(" Jane Doe "), QStringLiteral("jane@example.com") }),
            QStringLiteral("Jane Doe <jane@example.com>"));
        QCOMPARE(person({ { }, QStringLiteral("billing@example.com") }),
            QStringLiteral("billing@example.com"));
        QCOMPARE(person({ QStringLiteral("Only Name"), { } }), QStringLiteral("Only Name"));
        QCOMPARE(person({ }), QString());
        QCOMPARE(personMarkup({ QStringLiteral("Jane & Co"), QStringLiteral("jane@example.com") }),
            QStringLiteral("<b>Jane &amp; Co</b> <small><font "
                           "color=\"{dim}\">jane@example.com</font></small>"));
        QCOMPARE(personMarkup({ { }, QStringLiteral("<billing@example.com>") }),
            QStringLiteral("<b>&lt;billing@example.com&gt;</b>"));
    }

    void shortTimesStepFromNowToDates()
    {
        const QDateTime now = at("2026-09-16T16:00:00+02:00");
        const struct {
            const char* when;
            const char* en;
            const char* de;
        } cases[] = {
            { "2026-09-16T15:59:40+02:00", "now", "jetzt" },
            { "2026-09-16T15:55:00+02:00", "5 min ago", "vor 5 min" },
            { "2026-09-16T09:07:00+02:00", "09:07", "09:07" },
            { "2026-09-15T23:30:00+02:00", "yesterday", "gestern" },
            { "2026-09-13T10:00:00+02:00", "Sun", "So" },
            { "2026-06-08T10:00:00+02:00", "Jun 8", "8. Jun" },
            { "2025-09-15T10:00:00+02:00", "2025-09-15", "15.09.2025" },
        };
        for (const auto& c : cases) {
            QCOMPARE(shortTime(at(c.when), now, Lang::En), QString::fromUtf8(c.en));
            QCOMPARE(shortTime(at(c.when), now, Lang::De), QString::fromUtf8(c.de));
        }
    }

    void detailTimesAddTheClockBeforeToday()
    {
        const QDateTime now = at("2026-09-16T16:00:00+02:00");
        QCOMPARE(
            detailTime(at("2026-09-16T09:07:00+02:00"), now, Lang::En), QStringLiteral("09:07"));
        QCOMPARE(detailTime(at("2026-09-15T23:30:00+02:00"), now, Lang::De),
            QStringLiteral("gestern · 23:30"));
        QCOMPARE(detailTime(at("2026-06-08T10:00:00+02:00"), now, Lang::En),
            QStringLiteral("Jun 8 · 10:00"));
    }

    void daysAreTodayYesterdayOrOlder()
    {
        const QDate today(2026, 9, 16);
        QCOMPARE(dayIndex(at("2026-09-16T00:10:00+02:00"), today), 0);
        QCOMPARE(dayIndex(at("2026-09-17T09:00:00+02:00"), today), 0);
        QCOMPARE(dayIndex(at("2026-09-15T23:59:00+02:00"), today), 1);
        QCOMPARE(dayIndex(at("2026-09-14T12:00:00+02:00"), today), 2);
        QCOMPARE(dayIndex(std::nullopt, today), 2);
        QCOMPARE(QString::fromUtf8(texts(Lang::De).days[2]), QString::fromUtf8("Älter"));
    }

    void datesComeInThreeLengthsPerLanguage()
    {
        const QDate day(2026, 10, 3);
        const auto text = [&](Lang lang, int length) {
            return dateText(day, texts(lang).dateLengths[size_t(length)].format, lang);
        };
        QCOMPARE(text(Lang::De, 0), QStringLiteral("03.10.2026"));
        QCOMPARE(text(Lang::De, 1), QStringLiteral("3. Oktober 2026"));
        QCOMPARE(text(Lang::De, 2), QStringLiteral("Samstag, 3. Oktober 2026"));
        QCOMPARE(text(Lang::En, 0), QStringLiteral("2026-10-03"));
        QCOMPARE(text(Lang::En, 1), QStringLiteral("3 October 2026"));
        QCOMPARE(text(Lang::En, 2), QStringLiteral("Saturday, 3 October 2026"));
        const QVariantList dates = Strings().dates(QDateTime(day, QTime(12, 0)));
        QCOMPARE(dates.size(), 3);
        const QVariantMap longest = dates.at(2).toMap();
        QCOMPARE(longest.value(QStringLiteral("name")).toString(),
            QString::fromUtf8(texts().dateLengths[2].name));
        QCOMPARE(longest.value(QStringLiteral("text")).toString(), text(lang(), 2));
    }

    void sizesRoundUpToWholeUnits()
    {
        QCOMPARE(fileSize(1023), QStringLiteral("1023 B"));
        QCOMPARE(fileSize(120394), QStringLiteral("118 KB"));
        QCOMPARE(fileSize(25 * 1024 * 1024 + 1), QStringLiteral("26 MB"));
    }

    void downloadsGoToTheChosenFolderOrTheSystemOne()
    {
        const QString home = QDir::homePath();
        QCOMPARE(downloadsPath(QStringLiteral(" ~/Mail/ ")), home + QStringLiteral("/Mail"));
        QCOMPARE(downloadsPath(QStringLiteral("~")), home);
        QCOMPARE(downloadsPath(QStringLiteral("Mail")), home + QStringLiteral("/Mail"));
        QCOMPARE(downloadsPath(QStringLiteral("/srv/mail")), QStringLiteral("/srv/mail"));
        QCOMPARE(
            downloadsPath({ }), QStandardPaths::writableLocation(QStandardPaths::DownloadLocation));
        QCOMPARE(tildePath(home + QStringLiteral("/Downloads")), QStringLiteral("~/Downloads"));
        QCOMPARE(tildePath(home), QStringLiteral("~"));
        QCOMPARE(tildePath(home + QStringLiteral("x/y")), home + QStringLiteral("x/y"));
        QCOMPARE(tildePath(QStringLiteral("/srv/mail")), QStringLiteral("/srv/mail"));
    }

    void linksOpenWhatTheTextShows()
    {
        const QString text = QStringLiteral(
            "Hi Jane,\n\n  indented   twice\nSee https://example.com/a?b=1&c=2. Or "
            "<https://example.com/x>, (https://example.com/y) and "
            "https://en.example.org/wiki/Foo_(bar)!\nwww.example.com/path; not http:// nor www. "
            "<b>bold</b> & https://example.com/?q=a|b\nWrite to Paul <paul.berger@example.com>, "
            "anna.müller@example.de. or https://example.com/@me, not me@localhost nor a@b.c");
        QTextDocument document;
        const LinkedText linked = linkedText(text);
        document.setHtml(linked.html);
        QCOMPARE(document.toPlainText(), text);
        QVERIFY(linked.shortened.isEmpty());
        QCOMPARE(anchors(document),
            QStringList({
                QStringLiteral("https://example.com/a?b=1&c=2 = https://example.com/a?b=1&c=2"),
                QStringLiteral("https://example.com/x = https://example.com/x"),
                QStringLiteral("https://example.com/y = https://example.com/y"),
                QStringLiteral("https://en.example.org/wiki/Foo_(bar) = "
                               "https://en.example.org/wiki/Foo_(bar)"),
                QStringLiteral("https://www.example.com/path = www.example.com/path"),
                QStringLiteral("https://example.com/?q=a%7Cb = https://example.com/?q=a|b"),
                QStringLiteral("mailto:paul.berger@example.com = paul.berger@example.com"),
                QStringLiteral("mailto:anna.m%C3%BCller@example.de = anna.müller@example.de"),
                QStringLiteral("https://example.com/@me = https://example.com/@me"),
            }));
    }

    void longLinksAreShortenedBehindTheirHost()
    {
        const QString query = QStringLiteral("?token=") + QString(80, u'x');
        const QString longHost
            = QStringLiteral("https://") + QString(60, u'a') + QStringLiteral(".example.com");
        const QString mail = QString(60, u'b') + QStringLiteral("@example.com");
        const QString fits = QStringLiteral("https://example.com/") + QString(39, u'c');
        const QString onward = QStringLiteral("www.example.com/go?to=https://") + QString(60, u'd');
        const LinkedText linked = linkedText(
            QStringLiteral("Read <https://www.example.com/inbox.html%1>, %2/path/to/a/page "
                           "www.example.com/news%1 %3 %4 %5")
                .arg(query, longHost, mail, fits, onward));
        QTextDocument document;
        document.setHtml(linked.html);
        const QString inbox = QStringLiteral("https://www.example.com/inbox.html");
        const QString news = QStringLiteral("www.example.com/news");
        QCOMPARE(anchors(document),
            QStringList({
                inbox + query + QStringLiteral(" = ") + inbox
                    + QStringLiteral("?token=xxxxxxxxx\u2026"),
                longHost + QStringLiteral("/path/to/a/page = ") + longHost
                    + QStringLiteral("\u2026"),
                QStringLiteral("https://") + news + query + QStringLiteral(" = ") + news
                    + QStringLiteral("?token=") + QString(23, u'x') + QStringLiteral("\u2026"),
                QStringLiteral("mailto:") + mail + QStringLiteral(" = ") + mail,
                fits + QStringLiteral(" = ") + fits,
                QStringLiteral("https://") + onward + QStringLiteral(" = ") + onward.left(50)
                    + QStringLiteral("\u2026"),
            }));
        QCOMPARE(linked.shortened,
            QStringList({ inbox + query, longHost + QStringLiteral("/path/to/a/page"),
                QStringLiteral("https://") + news + query, QStringLiteral("https://") + onward }));
    }

    void mailtoLinksAskForANewMail()
    {
        Mailto mailto;
        QSignalSpy requested(&mailto, &Mailto::requested);
        QVERIFY(
            QDesktopServices::openUrl(QUrl(QStringLiteral("mailto:anna.m%C3%BCller@example.de"))));
        QCOMPARE(requested.count(), 1);
        QCOMPARE(requested.takeFirst().constFirst().toString(),
            QStringLiteral("anna.müller@example.de"));
        mailto.open(QUrl(QStringLiteral("mailto:")));
        QCOMPARE(requested.count(), 0);
    }

    void displayNamesPreferTheName()
    {
        QCOMPARE(displayName({ QStringLiteral(" Jane Doe "), QStringLiteral("jane@example.com") }),
            QStringLiteral("Jane Doe"));
        QCOMPARE(displayName({ { }, QStringLiteral("billing@example.com") }),
            QStringLiteral("billing@example.com"));
    }

    void receivedStringsParse()
    {
        QVERIFY(parseTime(QStringLiteral("2026-09-15T14:32:05+02:00")));
        QVERIFY(!parseTime(QStringLiteral("yesterday")));
    }

    void localesPickTheLanguage()
    {
        QCOMPARE(langFrom(QStringLiteral("de_DE.UTF-8")), Lang::De);
        QCOMPARE(langFrom(QStringLiteral("en_US.UTF-8")), Lang::En);
        QCOMPARE(langFrom(QStringLiteral("C")), Lang::En);
    }

    void hexColorsParse()
    {
        QCOMPARE(
            hexColor(QStringLiteral("#89b4fa")), std::optional<QColor>(QColor(0x89, 0xb4, 0xfa)));
        QCOMPARE(hexColor(QStringLiteral(" #000000 ")), std::optional<QColor>(QColor(0, 0, 0)));
        QVERIFY(!hexColor(QStringLiteral("89b4fa")));
        QVERIFY(!hexColor(QStringLiteral("#fff")));
        QVERIFY(!hexColor(QStringLiteral("#gggggg")));
    }

    void firstPresentKeyWinsThenFallback()
    {
        const Toml table = parseToml(QStringLiteral("accent = \"#112233\"\nblue = \"#445566\""));
        QCOMPARE(pickColor(table, { QStringLiteral("missing"), QStringLiteral("blue") },
                     QStringLiteral("#000000"))
                     .red(),
            0x44);
        QCOMPARE(
            pickColor(table, { QStringLiteral("nope") }, QStringLiteral("#0a0b0c")).blue(), 0x0c);
        QCOMPARE(
            pickColor({ }, { QStringLiteral("accent") }, QStringLiteral("#0a0b0c")).green(), 0x0b);
    }

    void tomlKeepsSectionsAndDropsComments()
    {
        const Toml table
            = parseToml(QStringLiteral("# theme\nmode = \"dark\"\n\n[font]\nbase-size = 13 "
                                       "# px\nfamily = \"A # B\"\n[bar.top]\nsize = 2\n"));
        QCOMPARE(table.value(QStringLiteral("mode")), QStringLiteral("dark"));
        QCOMPARE(table.value(QStringLiteral("font.base-size")), QStringLiteral("13"));
        QCOMPARE(table.value(QStringLiteral("font.family")), QStringLiteral("A # B"));
        QCOMPARE(table.value(QStringLiteral("bar.top.size")), QStringLiteral("2"));
    }

    void themePathsAreSeparated()
    {
        const QString theme = QStringLiteral("/home/x/.local/state/omarchy/current");
        QVERIFY(isInside(theme + QStringLiteral("/theme/colors.toml"), theme));
        QVERIFY(isInside(theme, theme));
        QVERIFY(!isInside(QStringLiteral("/home/x/.local/state/omarchy/currently"), theme));
        QVERIFY(!isInside(QStringLiteral("/m/private/inbox/a-1.json"), QString()));
    }

    void feedStreamsTheFirstLoadAndMergesReloads()
    {
        Feed feed;
        QAbstractItemModelTester tester(&feed);
        answer(mails({ "a", "b", "c" }));
        load(feed);
        QVERIFY(feed.streaming());
        QCOMPARE(subjects(feed), QStringList({ "a", "b", "c" }));
        QCOMPARE(feed.data(feed.index(0), Feed::IndentRole).toInt(), 0);
        QVERIFY(!feed.data(feed.index(0), Feed::FreshRole).toBool());
        QCOMPARE(feed.data(feed.index(1), Feed::PathRole).toString(),
            QStringLiteral("private/inbox/2026-09-15_1432-b.json"));
        QCOMPARE(feed.subject(QStringLiteral("private/inbox/2026-09-15_1432-b.json")),
            QStringLiteral("b"));
        QVERIFY(feed.subject(QStringLiteral("private/inbox/gone.json")).isEmpty());
        QCOMPARE(feed.data(feed.index(1), Feed::DayRole).toString(),
            QString::fromUtf8(texts().days[dayIndex(
                parseTime(QStringLiteral("2026-09-15T14:32:05+02:00")), QDate::currentDate())]));

        QSignalSpy inserted(&feed, &QAbstractItemModel::rowsInserted);
        QSignalSpy removed(&feed, &QAbstractItemModel::rowsRemoved);
        answer(mails({ "d", "a", "c" }));
        QElapsedTimer clock;
        clock.start();
        load(feed);
        const bool quick = clock.elapsed() < 1000;
        QVERIFY(!feed.streaming());
        QCOMPARE(subjects(feed), QStringList({ "d", "a", "c" }));
        QCOMPARE(inserted.count(), 1);
        QCOMPARE(inserted.first().at(1).toInt(), 0);
        QCOMPARE(removed.count(), 1);
        QCOMPARE(removed.first().at(1).toInt(), 1);
        QVERIFY(!quick || feed.data(feed.index(0), Feed::FreshRole).toBool());
        QVERIFY(!feed.data(feed.index(1), Feed::FreshRole).toBool());
        QCOMPARE(feed.count(), 3);
    }

    void feedFollowsAnySequenceOfReloads()
    {
        Feed feed;
        QAbstractItemModelTester tester(&feed);
        const QStringList pool { "a", "b", "c", "d", "e", "f", "g", "h" };
        QRandomGenerator random(42);
        for (int round = 0; round < 40; ++round) {
            QStringList ids;
            for (const QString& id : pool) {
                if (random.bounded(10) < 6)
                    ids.append(id);
            }
            std::shuffle(ids.begin(), ids.end(), random);
            answer(mails(ids));
            load(feed);
            QCOMPARE(subjects(feed), ids);
        }
    }

    void feedSearchesThroughLamemailWithoutMarkingHitsFresh()
    {
        Feed feed;
        QAbstractItemModelTester tester(&feed);
        const QString args = m_fake.filePath(QStringLiteral("args"));
        answer(mails({ "a", "b" }));
        load(feed);
        QVERIFY(read(args).endsWith("list\n--json\n"));

        QSignalSpy changed(&feed, &Feed::queryChanged);
        answer(mails({ "c", "b" }));
        feed.setQuery(QStringLiteral("jane doe"));
        QVERIFY(QTest::qWaitFor([&] { return !feed.loading(); }));
        QCOMPARE(changed.count(), 1);
        QVERIFY(read(args).endsWith("list\n--json\n--search\njane doe\n"));
        QCOMPARE(subjects(feed), QStringList({ "c", "b" }));
        QVERIFY(!feed.data(feed.index(0), Feed::FreshRole).toBool());
        feed.setQuery(QStringLiteral("jane doe"));
        QVERIFY(!feed.loading());

        answer(mails({ "d", "c", "b" }));
        QElapsedTimer clock;
        clock.start();
        load(feed);
        QVERIFY(clock.elapsed() >= 1000 || feed.data(feed.index(0), Feed::FreshRole).toBool());
        QVERIFY(read(args).endsWith("--search\njane doe\n"));

        answer(mails({ "d", "c", "b", "a" }));
        feed.setQuery(QStringLiteral("  "));
        QVERIFY(QTest::qWaitFor([&] { return !feed.loading(); }));
        QVERIFY(read(args).endsWith("list\n--json\n"));
        QCOMPARE(subjects(feed), QStringList({ "d", "c", "b", "a" }));
        QVERIFY(!feed.data(feed.index(3), Feed::FreshRole).toBool());
    }

    void feedListsItsKindsButSearchesEveryKind()
    {
        Feed feed;
        QAbstractItemModelTester tester(&feed);
        const QString args = m_fake.filePath(QStringLiteral("args"));
        feed.setKinds({ QStringLiteral("inbox"), QStringLiteral("sent") });
        answer(mails({ "a", "b" }));
        load(feed);
        QVERIFY(read(args).endsWith("list\n--json\n--kind\ninbox\n--kind\nsent\n"));

        answer({ });
        feed.setKinds({ QStringLiteral("trash") });
        QVERIFY(QTest::qWaitFor([&] { return !feed.loading(); }));
        QVERIFY(read(args).endsWith("list\n--json\n--kind\ntrash\n"));
        QCOMPARE(feed.count(), 0);

        answer(mails({ "c", "a", "b" }));
        feed.setKinds({ QStringLiteral("archive") });
        QVERIFY(QTest::qWaitFor([&] { return !feed.loading(); }));
        QVERIFY(read(args).endsWith("list\n--json\n--kind\narchive\n"));
        QCOMPARE(subjects(feed), QStringList({ "c", "a", "b" }));
        QVERIFY(!feed.data(feed.index(0), Feed::FreshRole).toBool());

        answer(mails({ "d" }));
        feed.setQuery(QStringLiteral("jane"));
        QVERIFY(QTest::qWaitFor([&] { return !feed.loading(); }));
        QVERIFY(read(args).endsWith("list\n--json\n--search\njane\n"));
    }

    void feedPinsStarredMailAndMovesRowsWithoutGlow()
    {
        Feed feed;
        QAbstractItemModelTester tester(&feed);
        const QString args = m_fake.filePath(QStringLiteral("args"));
        answer(mails({ "a", "b", "c", "d" }));
        load(feed);
        feed.setFlaggedFirst(true);
        QVERIFY(QTest::qWaitFor([&] { return !feed.loading(); }));
        QVERIFY(read(args).endsWith("list\n--json\n--flagged-first\n"));

        QSignalSpy inserted(&feed, &QAbstractItemModel::rowsInserted);
        QSignalSpy removed(&feed, &QAbstractItemModel::rowsRemoved);
        answer(pinned("c") + mails({ "a", "b", "d" }));
        load(feed);
        QCOMPARE(subjects(feed), QStringList({ "c", "a", "b", "d" }));
        QCOMPARE(inserted.count(), 1);
        QCOMPARE(removed.count(), 1);
        QVERIFY(!feed.data(feed.index(0), Feed::FreshRole).toBool());
        QCOMPARE(
            feed.data(feed.index(0), Feed::DayRole).toString(), QString::fromUtf8(texts().starred));
        QVERIFY(feed.data(feed.index(1), Feed::DayRole).toString()
            != QString::fromUtf8(texts().starred));

        inserted.clear();
        removed.clear();
        answer(mails({ "a", "b", "c", "d" }));
        load(feed);
        QCOMPARE(subjects(feed), QStringList({ "a", "b", "c", "d" }));
        QCOMPARE(inserted.count(), 1);
        QCOMPARE(removed.count(), 1);
        QVERIFY(!feed.data(feed.index(2), Feed::FreshRole).toBool());

        feed.setFlaggedFirst(false);
        QVERIFY(QTest::qWaitFor([&] { return !feed.loading(); }));
        QVERIFY(read(args).endsWith("list\n--json\n"));
    }

    void feedRepaintsARowWhoseAccountColourChanged()
    {
        Feed feed;
        QAbstractItemModelTester tester(&feed);
        answer(mail(QStringLiteral("a")));
        load(feed);
        QCOMPARE(
            feed.data(feed.index(0), Feed::AccountColorRole).toString(), QStringLiteral("blue"));
        QSignalSpy changed(&feed, &QAbstractItemModel::dataChanged);
        answer(mail(QStringLiteral("a"), true, "orange"));
        load(feed);
        QCOMPARE(changed.count(), 1);
        QCOMPARE(
            feed.data(feed.index(0), Feed::AccountColorRole).toString(), QStringLiteral("orange"));
    }

    void feedCountsUnread()
    {
        Feed feed;
        QAbstractItemModelTester tester(&feed);
        answer(mail(QStringLiteral("a"), false) + mail(QStringLiteral("b")));
        load(feed);
        QCOMPARE(subjects(feed), QStringList({ "a", "b" }));
        QCOMPARE(feed.unread(), 1);
        QSignalSpy changed(&feed, &QAbstractItemModel::dataChanged);
        answer(mail(QStringLiteral("a")) + mail(QStringLiteral("b")));
        load(feed);
        QCOMPARE(feed.unread(), 0);
        QCOMPARE(changed.count(), 1);
        QCOMPARE(changed.first().at(0).value<QModelIndex>().row(), 0);
    }

    void feedMarksAtOnceAndReloadsWhenTheJournalHasIt()
    {
        Feed feed;
        QAbstractItemModelTester tester(&feed);
        answer(mail(QStringLiteral("a"), false) + mail(QStringLiteral("b")));
        load(feed);
        QCOMPARE(feed.unread(), 1);
        const QString path = feed.data(feed.index(0), Feed::PathRole).toString();
        const QString log = m_fake.filePath(QStringLiteral("log"));
        const QString sleep = m_fake.filePath(QStringLiteral("sleep-flag"));
        write(log, { });
        write(sleep, "0.3");
        auto awake = qScopeGuard([&] { QFile::remove(sleep); });

        feed.markSeen(path, true);
        QVERIFY(feed.data(feed.index(0), Feed::SeenRole).toBool());
        QCOMPARE(feed.unread(), 0);
        load(feed);
        QVERIFY(feed.data(feed.index(0), Feed::SeenRole).toBool());
        feed.markFlagged(path, true);
        QVERIFY(feed.data(feed.index(0), Feed::FlaggedRole).toBool());
        answer(mail(QStringLiteral("a")) + mail(QStringLiteral("b")));
        const QByteArray flagged = "flag\n" + path.toUtf8() + "\nflagged\n";
        QTRY_VERIFY_WITH_TIMEOUT(read(log).contains(flagged)
                && read(log).lastIndexOf("list\n--json") > read(log).indexOf(flagged),
            5000);
        QVERIFY(
            read(log).indexOf("flag\n" + path.toUtf8() + "\nseen\n") < read(log).indexOf(flagged));
        QVERIFY(QTest::qWaitFor([&] { return !feed.loading(); }));
        QVERIFY(feed.data(feed.index(0), Feed::SeenRole).toBool());

        write(log, { });
        feed.markSeen(path, true);
        QTest::qWait(100);
        QVERIFY(read(log).isEmpty());
    }

    void feedPutsAFailedMarkBack()
    {
        Feed feed;
        QAbstractItemModelTester tester(&feed);
        answer(mail(QStringLiteral("a")));
        load(feed);
        const QString path = feed.data(feed.index(0), Feed::PathRole).toString();
        QSignalSpy failed(&feed, &Feed::actionFailed);
        connect(&feed, &Feed::actionFailed, this, [this] { answer(mail(QStringLiteral("a"))); });
        answer({ }, 1, "lamemail: the message is no longer on the server\n");

        feed.markFlagged(path, true);
        QVERIFY(feed.data(feed.index(0), Feed::FlaggedRole).toBool());
        QTRY_COMPARE(failed.count(), 1);
        QCOMPARE(failed.first().at(0).toString(), path);
        QCOMPARE(failed.first().at(1).toString(),
            QStringLiteral("the message is no longer on the server"));
        QVERIFY(QTest::qWaitFor([&] { return !feed.loading(); }));
        QVERIFY(!feed.data(feed.index(0), Feed::FlaggedRole).toBool());
    }

    void feedRestylesAMovedRow()
    {
        Feed feed;
        QAbstractItemModelTester tester(&feed);
        answer(mails({ QStringLiteral("a"), QStringLiteral("b") }));
        load(feed);
        const QString path = feed.data(feed.index(0), Feed::PathRole).toString();
        const QString log = m_fake.filePath(QStringLiteral("log"));
        write(log, { });
        QSignalSpy moved(&feed, &Feed::moved);
        answer(
            QStringLiteral(
                R"({"path":"%1","from":"inbox","folder":"archive","style":{"icon":"archive","dense":false,"dim":false,"strike":false,"italic":false},"kind":"archive"})")
                .arg(path)
                .toUtf8()
            + '\n');
        QStringList shown;
        connect(&feed, &Feed::moved, this, [&] {
            shown << QString::number(feed.rowCount())
                  << feed.data(feed.index(0), Feed::IconRole).toString()
                  << feed.data(feed.index(0), Feed::KindRole).toString();
            answer(mails({ QStringLiteral("a"), QStringLiteral("b") }));
        });

        feed.move(path, QStringLiteral("archive"));
        QTRY_COMPARE(moved.count(), 1);
        QCOMPARE(shown, QStringList({ "2", "archive", "archive" }));
        QCOMPARE(moved.first(),
            (QVariantList { path, QStringLiteral("inbox"), QStringLiteral("archive"),
                QStringLiteral("archive") }));
        QVERIFY(read(log).contains("move\n--json\n" + path.toUtf8() + "\narchive\n"));
        QVERIFY(QTest::qWaitFor([&] { return !feed.loading(); }));
        answer({ });
    }

    void feedDoesNotGlowForAMailThatMoved()
    {
        Feed feed;
        QAbstractItemModelTester tester(&feed);
        answer(mails({ QStringLiteral("a"), QStringLiteral("b") }));
        load(feed);
        answer(mail(QStringLiteral("a")).replace("private/inbox/", "private/archive/")
            + mail(QStringLiteral("b")));
        load(feed);
        QCOMPARE(feed.data(feed.index(0), Feed::PathRole).toString(),
            QStringLiteral("private/archive/2026-09-15_1432-a.json"));
        QVERIFY(!feed.data(feed.index(0), Feed::FreshRole).toBool());
    }

    void feedKeepsItsRowsWhenLamemailFails()
    {
        Feed feed;
        QAbstractItemModelTester tester(&feed);
        answer(mails({ "a", "b" }));
        load(feed);
        answer({ }, 1, "lamemail: no settings.json in /x; run `lamemail init` first\n");
        load(feed);
        QCOMPARE(feed.error(), QStringLiteral("no settings.json in /x; run `lamemail init` first"));
        QCOMPARE(subjects(feed), QStringList({ "a", "b" }));
        answer(mails({ "b" }));
        load(feed);
        QVERIFY(feed.error().isEmpty());
        QCOMPARE(subjects(feed), QStringList({ "b" }));
    }

    void feedDropsAFirstLoadThatFails()
    {
        Feed feed;
        QAbstractItemModelTester tester(&feed);
        answer(mails({ "a", "b" }), 3);
        load(feed);
        QCOMPARE(feed.rowCount(), 0);
        QCOMPARE(feed.count(), 0);
        QCOMPARE(feed.error(),
            QString::fromUtf8(texts().exitedWithCode)
                .replace(QLatin1String("{}"), QLatin1String("3")));
    }

    void syncerSummarizesOrReportsTheFailure()
    {
        Syncer syncer;
        QSignalSpy finished(&syncer, &Syncer::finished);
        QList<QVariantMap> progress;
        connect(
            &syncer, &Syncer::progressChanged, this, [&] { progress.append(syncer.progress()); });
        answer(
            R"({"account":"business","folder":"inbox","fetched":50,"of":120,"folder_number":2,"folders":5})"
            "\n"
            R"({"account":"business","new":2,"removed":0,"updated":1})"
            "\n"
            R"({"account":"private","new":1,"removed":3,"updated":0})"
            "\n");
        syncer.sync();
        QVERIFY(syncer.running());
        QTRY_COMPARE(finished.count(), 1);
        QVERIFY(!syncer.running());
        QCOMPARE(finished.first().at(0).toBool(), true);
        const QVariantMap counts = finished.first().at(1).toMap();
        QCOMPARE(counts.value(QStringLiteral("accounts")).toInt(), 2);
        QCOMPARE(counts.value(QStringLiteral("added")).toInt(), 3);
        QCOMPARE(counts.value(QStringLiteral("removed")).toInt(), 3);
        QCOMPARE(counts.value(QStringLiteral("updated")).toInt(), 1);
        QCOMPARE(progress.size(), 2);
        QCOMPARE(
            progress.first().value(QStringLiteral("folder")).toString(), QStringLiteral("inbox"));
        QCOMPARE(progress.first().value(QStringLiteral("of")).toInt(), 120);
        QCOMPARE(progress.first().value(QStringLiteral("folderNumber")).toInt(), 2);
        QVERIFY(syncer.progress().isEmpty());

        answer(R"({"account":"private","error":"login failed"})"
               "\n",
            1, "lamemail: sync incomplete, see above\n");
        syncer.sync();
        QTRY_COMPARE(finished.count(), 2);
        QCOMPARE(finished.last().at(0).toBool(), false);
        QCOMPARE(finished.last().at(1).toMap().value(QStringLiteral("failed")).toInt(), 1);
        QCOMPARE(finished.last().at(2).toString(), QStringLiteral("private: login failed"));

        answer({ }, 1, "lamemail: another sync is running\n");
        syncer.sync();
        QTRY_COMPARE(finished.count(), 3);
        QCOMPARE(finished.last().at(2).toString(), QStringLiteral("another sync is running"));
    }

    void syncerCatchesUpOnASyncAskedForWhileRunning()
    {
        Syncer syncer;
        QSignalSpy finished(&syncer, &Syncer::finished);
        write(m_fake.filePath(QStringLiteral("log")), { });
        answer(R"({"account":"private","new":0,"removed":0,"updated":0})"
               "\n");
        syncer.sync();
        syncer.sync({ QStringLiteral("work") });
        syncer.sync({ QStringLiteral("work"), QStringLiteral("home") });
        QTRY_COMPARE(finished.count(), 2);
        QVERIFY(!syncer.running());
        const QByteArray log = read(m_fake.filePath(QStringLiteral("log")));
        QVERIFY(log.contains("sync\n--json\nstdin:"));
        QVERIFY(log.contains("sync\n--json\nwork\nhome\nstdin:"));
        answer({ });
    }

    void parsesTheWatchLines()
    {
        const auto synced = watchLineFromJson(
            R"({"account":"private","new":1,"removed":0,"updated":0,"arrived":[{"path":"private/inbox/2026-09-15_1432-offer-for-project-4711.json","from":[{"name":"Jane Doe","address":"jane@example.com"}],"subject":"Offer for project"}]})");
        QVERIFY(synced);
        QCOMPARE(synced->account, QStringLiteral("private"));
        QVERIFY(synced->error.isEmpty());
        QCOMPARE(synced->arrived.size(), 1);
        QCOMPARE(synced->arrived.first().path,
            QStringLiteral("private/inbox/2026-09-15_1432-offer-for-project-4711.json"));
        QCOMPARE(synced->arrived.first().from.first().name, QStringLiteral("Jane Doe"));
        QCOMPARE(synced->arrived.first().subject, QStringLiteral("Offer for project"));

        const auto retrying = watchLineFromJson(
            R"({"account":"business","error":"connecting to imap.example.com:993 timed out","retry":30})");
        QVERIFY(retrying && retrying->retrying);
        QCOMPARE(retrying->error, QStringLiteral("connecting to imap.example.com:993 timed out"));
        const auto stopped = watchLineFromJson(
            R"({"account":"work","error":"login as work@example.com failed","retry":null})");
        QVERIFY(stopped && !stopped->retrying && !stopped->syncing);
        const auto started = watchLineFromJson(R"({"account":"private","syncing":true})");
        QVERIFY(started && started->syncing && started->error.isEmpty());
    }

    void mailWatchReportsArrivalsAndStoppedAccounts()
    {
        MailWatch watch;
        QSignalSpy arrived(&watch, &MailWatch::arrived);
        QSignalSpy stopped(&watch, &MailWatch::stopped);
        QSignalSpy synced(&watch, &MailWatch::synced);
        QList<QStringList> syncing;
        connect(&watch, &MailWatch::stateChanged, this, [&] { syncing.append(watch.syncing()); });
        write(m_fake.filePath(QStringLiteral("log")), { });
        QList<QVariantMap> progress;
        connect(&watch, &MailWatch::stateChanged, this, [&] { progress.append(watch.progress()); });
        answer(
            R"({"account":"private","syncing":true})"
            "\n"
            R"({"account":"private","folder":"sent","fetched":0,"of":3,"folder_number":1,"folders":4})"
            "\n"
            R"({"account":"private","new":0,"removed":0,"updated":0,"arrived":[]})"
            "\n"
            R"({"account":"business","syncing":true})"
            "\n"
            R"({"account":"business","error":"no route to host","retry":30})"
            "\n"
            R"({"account":"private","new":2,"removed":0,"updated":0,"arrived":[{"path":"private/inbox/a-7.json","from":[{"name":"Jane Doe","address":"jane@example.com"}],"subject":"Offer"},{"path":"private/inbox/b-8.json","from":[],"subject":""}]})"
            "\n"
            R"({"account":"work","error":"login as work@example.com failed","retry":null})"
            "\n"
            R"({"account":"private","syncing":true})"
            "\n",
            1);
        QTest::ignoreMessage(
            QtWarningMsg, "lamemail-ui: lamemail watch: business: no route to host");
        watch.start();
        QTRY_COMPARE(stopped.count(), 1);
        QCOMPARE(stopped.first().at(0).toString(),
            QStringLiteral("work: login as work@example.com failed"));
        QCOMPARE(arrived.count(), 1);
        const QVariantList mails = arrived.first().at(0).toList();
        QCOMPARE(mails.size(), 2);
        QCOMPARE(mails.at(0).toMap().value(QStringLiteral("path")).toString(),
            QStringLiteral("private/inbox/a-7.json"));
        QCOMPARE(mails.at(0).toMap().value(QStringLiteral("sender")).toString(),
            QStringLiteral("Jane Doe"));
        QCOMPARE(mails.at(1).toMap().value(QStringLiteral("sender")).toString(), QString());
        QCOMPARE(synced.count(), 2);
        QCOMPARE(
            watch.failing(), (QStringList { QStringLiteral("business"), QStringLiteral("work") }));
        QCOMPARE(watch.failures(),
            QStringLiteral("business: no route to host\nwork: login as work@example.com failed"));
        QVERIFY(syncing.contains(QStringList { QStringLiteral("private") }));
        QVERIFY(std::any_of(progress.begin(), progress.end(), [](const QVariantMap& step) {
            return step.value(QStringLiteral("folder")) == QStringLiteral("sent");
        }));
        QTRY_VERIFY(watch.syncing().isEmpty());
        QVERIFY(watch.progress().isEmpty());
        QVERIFY(read(m_fake.filePath(QStringLiteral("log"))).contains("watch\n--json\nstdin:"));
        answer({ });
    }

    void journalCountsItsEntriesKnowsTheirPathsAndReportsWhatAPushDroppedOrSent()
    {
        const auto result = pushResultFromJson(
            R"({"account":"private","pushed":2,"dropped":[{"path":"private/inbox/a-7.json","reason":"gone"},{"message_id":"m@example.com","reason":"refused"}],"sent":[{"account":"private","message_id":"n@example.com","to":[{"name":"Jane Doe","address":"jane@example.com"}],"cc":[],"bcc":[{"name":null,"address":"tom@example.com"}],"copy":"failed","folder":null,"error":"no copy"}]})");
        QVERIFY(result);
        QCOMPARE(result->dropped.size(), 2);
        QCOMPARE(result->dropped.first().reason, QStringLiteral("gone"));
        QVERIFY(result->dropped.first().messageId.isEmpty());
        QCOMPARE(result->dropped.last().messageId, QStringLiteral("m@example.com"));
        QCOMPARE(result->sent.size(), 1);
        QCOMPARE(result->sent.first().messageId, QStringLiteral("n@example.com"));
        QCOMPARE(result->sent.first().people.size(), 2);
        QCOMPARE(result->sent.first().error, QStringLiteral("no copy"));

        Journal journal;
        QSignalSpy dropped(&journal, &Journal::dropped);
        QSignalSpy sent(&journal, &Journal::sent);
        const QString log = m_fake.filePath(QStringLiteral("log"));
        write(log, { });
        answer(
            R"({"id":0,"path":"private/inbox/a-7.json","uidvalidity":1,"action":"flag","change":"seen"})"
            "\n"
            R"({"id":1,"path":"private/inbox/b-8.json","uidvalidity":1,"action":"flag","change":"flagged"})"
            "\n"
            R"({"action":"send","id":2,"account":"private","message_id":"n@example.com","to":[]})"
            "\n"
            R"({"id":3,"path":"private/inbox/c-9.json","uidvalidity":1,"action":"move","folder":"archive"})"
            "\n"
            R"({"id":4,"path":"private/drafts/d-3.json","uidvalidity":1,"action":"discard"})"
            "\n"
            R"({"action":"send","id":5,"account":"private","path":"private/inbox/a-7.json","message_id":"r@example.com","to":[]})"
            "\n");
        journal.reload();
        QTRY_COMPARE(journal.count(), 4);
        QCOMPARE(journal.sending(), 2);
        QCOMPARE(journal.leaving(), QStringList { QStringLiteral("private/inbox/c-9.json") });
        QCOMPARE(journal.answering(), QStringList { QStringLiteral("private/inbox/a-7.json") });
        answer(
            R"({"account":"private","pushed":2,"dropped":[{"path":"private/inbox/b-8.json","reason":"the message is no longer on the server"}],"sent":[{"account":"private","message_id":"n@example.com","to":[{"name":"Jane Doe","address":"jane@example.com"}],"cc":[],"bcc":[],"copy":"saved","folder":"private/sent"}]})"
            "\n");
        journal.push();
        QVERIFY(journal.pushing());
        journal.push();
        QTRY_COMPARE(dropped.count(), 2);
        QCOMPARE(dropped.first().at(0).toString(),
            QStringLiteral("the message is no longer on the server"));
        QVERIFY(dropped.first().at(1).toString().isEmpty());
        QCOMPARE(sent.count(), 2);
        QCOMPARE(sent.first().at(0).toString(), QStringLiteral("n@example.com"));
        QCOMPARE(sent.first().at(1).toString(),
            QString::fromUtf8(texts().sentTo)
                .replace(QLatin1String("{}"), QLatin1String("Jane Doe")));
        QVERIFY(sent.first().at(2).toBool());
        QVERIFY(read(log).contains("journal\n--json\nstdin:"));
        QCOMPARE(read(log).count("push\n--json\nstdin:"), 2);
        QTRY_VERIFY(!journal.pushing());
        QVERIFY(journal.failure().isEmpty());

        answer(R"({"account":"private","error":"connecting to imap.example.com:993 timed out"})"
               "\n",
            1, "lamemail: push incomplete, see above\n");
        journal.push();
        QTRY_VERIFY(!journal.failure().isEmpty());
        QCOMPARE(journal.failure(),
            QStringLiteral("private: connecting to imap.example.com:993 timed out"));
        answer({ });
        journal.reload();
        QTRY_VERIFY(journal.failure().isEmpty());
        QCOMPARE(journal.sending(), 0);
        QVERIFY(journal.leaving().isEmpty());
        QVERIFY(journal.answering().isEmpty());

        QSignalSpy changed(&journal, &Journal::changed);
        answer(
            R"({"id":6,"path":"private/inbox/e-1.json","uidvalidity":1,"action":"move","folder":"archive"})"
            "\n");
        journal.reload();
        QTRY_COMPARE(changed.count(), 1);
        answer(
            R"({"id":7,"path":"private/inbox/f-2.json","uidvalidity":1,"action":"move","folder":"archive"})"
            "\n");
        journal.reload();
        QTRY_COMPARE(changed.count(), 2);
        QCOMPARE(journal.count(), 1);
        QCOMPARE(journal.leaving(), QStringList { QStringLiteral("private/inbox/f-2.json") });
        answer({ });
        journal.reload();
        QTRY_VERIFY(journal.leaving().isEmpty());
    }

    void notesComeOneByOneOrTogether()
    {
        const auto mail = [](const char* sender, const char* subject, const char* path) {
            return QVariantMap { { QStringLiteral("sender"), QString::fromUtf8(sender) },
                { QStringLiteral("subject"), QString::fromUtf8(subject) },
                { QStringLiteral("path"), QString::fromUtf8(path) } };
        };
        const QVariantList two { mail("Jane Doe", "Offer", "a"), mail("Paul", "Lunch", "b") };
        QCOMPARE(notes(two, Lang::En),
            (QList<Note> {
                { QStringLiteral("Jane Doe"), QStringLiteral("Offer"), QStringLiteral("a") },
                { QStringLiteral("Paul"), QStringLiteral("Lunch"), QStringLiteral("b") } }));
        const QVariantList four
            = two + QVariantList { mail("Jane Doe", "Again", "c"), mail("", "?", "d") };
        QCOMPARE(notes(four, Lang::De),
            (QList<Note> {
                { QStringLiteral("4 neue Mails"), QStringLiteral("Jane Doe, Paul"), { } } }));
        QVERIFY(notes({ }, Lang::En).isEmpty());
    }

    void trayImageCarriesADotInTheGivenColour()
    {
        QSize size;
        const QImage image = TrayImages().requestImage(QStringLiteral("112233"), &size, { 64, 64 });
        QCOMPARE(size, QSize(64, 64));
        QCOMPARE(image.pixelColor(51, 15), QColor(0x11, 0x22, 0x33));
        QCOMPARE(image.pixelColor(37, 10).alpha(), 0);
        QCOMPARE(image.pixelColor(20, 10).alpha(), 255);
    }

    void movesFollowTheKindOfTheFolder()
    {
        const Styles styles;
        const QString inbox = QStringLiteral("inbox");
        const QString archive = QStringLiteral("archive");
        const QString trash = QStringLiteral("trash");
        for (const QString& kind : { QString(), inbox, QStringLiteral("sent") })
            QCOMPARE(styles.moves(kind), QStringList({ archive, trash }));
        QCOMPARE(styles.moves(archive), QStringList({ inbox, trash }));
        QCOMPARE(styles.moves(QStringLiteral("junk")), QStringList({ inbox, trash }));
        QCOMPARE(styles.moves(trash), QStringList({ inbox }));
        QCOMPARE(styles.moves(QStringLiteral("drafts")), QStringList({ trash }));
        QCOMPARE(styles.kindIcon(inbox), QStringLiteral("incoming"));
        QCOMPARE(styles.kindIcon(QStringLiteral("sent")), QStringLiteral("outgoing"));
        QCOMPARE(styles.kindIcon(trash), trash);
        QCOMPARE(styles.kindIcon(QStringLiteral("other")), QStringLiteral("mail"));
        QCOMPARE(styles.kindLabel(QStringLiteral("sent")), QString::fromUtf8(texts().icons[1]));
        QCOMPARE(styles.kindLabel(QStringLiteral("other")), QString::fromUtf8(texts().kindOther));
        QCOMPARE(styles.kinds().last(), QStringLiteral("other"));
        QCOMPARE(styles.flags().size(), qsizetype(texts().flags.size()));
        QCOMPARE(styles.iconLabel(QStringLiteral("incoming")), QString::fromUtf8(texts().icons[0]));
    }

    void mailShowsOneMessageFromShowJson()
    {
        Mail mail;
        answer(
            R"({"account":"private","color":"cyan","kind":"trash","path":"private/inbox/x-1.json","received":"2026-09-15T14:32:05+02:00","from":[{"name":"Jane Doe","address":"jane@example.com"}],"to":[{"name":null,"address":"me@example.com"}],"cc":[{"name":"Paul","address":"paul@example.com"}],"bcc":[{"name":null,"address":"tom@example.com"}],"subject":"Offer","message_id":"o1@example.com","body":"Hi\n\nJane\n\n","flags":{"seen":true,"answered":false,"flagged":true,"draft":false,"keywords":[]},"answer_from":{"name":"Me","address":"me@example.com"},"answer_to":[{"name":"Jane Doe","address":"jane@example.com"}],"answer_cc":[{"name":"Paul","address":"paul@example.com"},{"name":null,"address":"maria@example.com"}],"answer_subject":"Re: Offer","forward_subject":"Fwd: Offer","quote":"Jane wrote:\n> Hi\n","signature":"Me\nexample.com","parent":"private/inbox/p-2.json","parts":[{"kind":"text","text":"Hi"},{"kind":"quote","attribution":"Jane wrote","parts":[{"kind":"text","text":"Old"}]}],"attachments":[{"index":0,"name":"offer.pdf","type":"application/pdf","size":120394},{"index":1,"name":null,"type":"image/png","size":10}]})"
            "\n");
        QString loaded;
        connect(&mail, &Mail::loadingChanged, this, [&] {
            if (!mail.loading())
                loaded = mail.subject();
        });
        mail.open(QStringLiteral("private/inbox/x-1.json"));
        QVERIFY(mail.loading());
        QVERIFY(QTest::qWaitFor([&] { return !mail.loading(); }));
        QVERIFY(mail.error().isEmpty());
        QCOMPARE(loaded, QStringLiteral("Offer"));
        QCOMPARE(mail.sender(), QStringLiteral("Jane Doe <jane@example.com>"));
        QCOMPARE(mail.recipients(), QStringLiteral("me@example.com, Paul <paul@example.com>"));
        QCOMPARE(mail.answerFrom(), QStringLiteral("me@example.com"));
        QCOMPARE(mail.answerSubject(), QStringLiteral("Re: Offer"));
        QCOMPARE(mail.forwardSubject(), QStringLiteral("Fwd: Offer"));
        QCOMPARE(mail.quote(), QStringLiteral("Jane wrote:\n> Hi\n"));
        QCOMPARE(mail.signature(), QStringLiteral("Me\nexample.com"));
        const QVariantMap draft = mail.draft();
        QCOMPARE(
            draft.value(QStringLiteral("draftId")).toString(), QStringLiteral("o1@example.com"));
        QCOMPARE(
            draft.value(QStringLiteral("from")).toString(), QStringLiteral("jane@example.com"));
        QCOMPARE(draft.value(QStringLiteral("text")).toString(), QStringLiteral("Hi\n\nJane"));
        QCOMPARE(draft.value(QStringLiteral("parent")).toString(),
            QStringLiteral("private/inbox/p-2.json"));
        const QVariantList people = draft.value(QStringLiteral("people")).toList();
        QCOMPARE(people.size(), 3);
        QCOMPARE(
            people.at(1).toMap().value(QStringLiteral("kind")).toString(), QStringLiteral("cc"));
        QCOMPARE(people.at(2).toMap().value(QStringLiteral("address")).toString(),
            QStringLiteral("tom@example.com"));
        QCOMPARE(
            people.at(2).toMap().value(QStringLiteral("kind")).toString(), QStringLiteral("bcc"));
        QCOMPARE(mail.answerTo().size(), 1);
        QCOMPARE(mail.answerTo().first().toMap().value(QStringLiteral("name")),
            QVariant(QStringLiteral("Jane Doe")));
        QCOMPARE(mail.answerCc().size(), 2);
        const QVariantMap maria = mail.answerCc().last().toMap();
        QVERIFY(maria.value(QStringLiteral("name")).toString().isEmpty());
        QCOMPARE(
            maria.value(QStringLiteral("address")), QVariant(QStringLiteral("maria@example.com")));
        QCOMPARE(mail.account(), QStringLiteral("private"));
        QCOMPARE(mail.color(), QStringLiteral("cyan"));
        QCOMPARE(mail.kind(), QStringLiteral("trash"));
        QCOMPARE(mail.parts().size(), 2);
        QVERIFY(mail.parts()
                .first()
                .toMap()
                .value(QStringLiteral("html"))
                .toString()
                .contains(QLatin1String(">Hi<")));
        const QVariantMap quote = mail.parts().at(1).toMap();
        QCOMPARE(quote.value(QStringLiteral("kind")).toString(), QStringLiteral("quote"));
        const QVariantMap old = quote.value(QStringLiteral("parts")).toList().first().toMap();
        QCOMPARE(old.value(QStringLiteral("text")), QVariant(QStringLiteral("Old")));
        QVERIFY(old.value(QStringLiteral("html")).toString().contains(QLatin1String(">Old<")));
        QCOMPARE(mail.attachments().first().toMap().value(QStringLiteral("size")),
            QVariant(QStringLiteral("118 KB")));
        QCOMPARE(mail.attachments().last().toMap().value(QStringLiteral("name")),
            QVariant(QStringLiteral("image/png")));
        QVERIFY(mail.flagged());

        QSignalSpy changed(&mail, &Mail::changed);
        QSignalSpy flagged(&mail, &Mail::flaggedChanged);
        mail.setFlagged(false);
        mail.reload();
        QVERIFY(QTest::qWaitFor([&] { return !mail.loading(); }));
        QCOMPARE(changed.count(), 0);
        QCOMPARE(flagged.count(), 2);
        QVERIFY(mail.flagged());

        answer({ }, 1, "lamemail: `x` is not a message file below /sync\n");
        mail.reload();
        QVERIFY(QTest::qWaitFor([&] { return !mail.loading(); }));
        QCOMPARE(mail.error(), QStringLiteral("`x` is not a message file below /sync"));
        QCOMPARE(mail.subject(), QStringLiteral("Offer"));

        mail.open(QStringLiteral("private/inbox/y-2.json"));
        QVERIFY(mail.subject().isEmpty());
        QVERIFY(QTest::qWaitFor([&] { return !mail.loading(); }));
    }

    void mailOpensAnAttachmentOnceThroughTheCli()
    {
        Mail mail;
        const QString path = QStringLiteral("private/inbox/x-1.json");
        answer(
            R"({"subject":"Offer","attachments":[{"index":0,"name":"offer.pdf","type":"application/pdf","size":13}]})"
            "\n");
        mail.open(path);
        QVERIFY(QTest::qWaitFor([&] { return !mail.loading(); }));
        QCOMPARE(mail.attachments().first().toMap().value(QStringLiteral("index")), QVariant(0));

        QSignalSpy ready(&mail, &Mail::attachmentReady);
        QSignalSpy failed(&mail, &Mail::attachmentFailed);
        const QString saved = QStringLiteral("/run/user/1000/lamemail/attachments/x-1/0/offer.pdf");
        answer(
            R"({"index":0,"name":"offer.pdf","type":"application/pdf","size":13,"path":"/run/user/1000/lamemail/attachments/x-1/0/offer.pdf"})"
            "\n");
        const QString log = m_fake.filePath(QStringLiteral("log"));
        const QString sleep = m_fake.filePath(QStringLiteral("sleep-attachment"));
        write(log, { });
        write(sleep, "0.3");
        auto awake = qScopeGuard([&] { QFile::remove(sleep); });
        mail.openAttachment(0);
        mail.openAttachment(0);
        QCOMPARE(mail.opening(), QVariantList { 0 });
        QTRY_COMPARE(ready.count(), 1);
        QVERIFY(mail.opening().isEmpty());
        QCOMPARE(ready.takeFirst().first().toUrl(), QUrl::fromLocalFile(saved));
        QCOMPARE(read(log).count("\nattachment\n"), 1);
        QCOMPARE(read(m_fake.filePath(QStringLiteral("args"))),
            "--dir\n" + lamemail::dir().toUtf8() + "\nattachment\n--json\n--\n" + path.toUtf8()
                + "\n0\n");

        answer({ }, 1, "lamemail: the message is no longer on the server\n");
        mail.openAttachment(0);
        mail.open(QStringLiteral("private/inbox/y-2.json"));
        QVERIFY(mail.opening().isEmpty());
        QTRY_COMPARE(failed.count(), 1);
        QCOMPARE(failed.first().first().toString(),
            QStringLiteral("the message is no longer on the server"));
        QVERIFY(ready.isEmpty());
        QVERIFY(QTest::qWaitFor([&] { return !mail.loading(); }));
    }

    void openerFallsBackToTheDesktopWithoutThePortal()
    {
        QCOMPARE(portalRequestPath(QStringLiteral(":1.42"), QStringLiteral("lamemail1")),
            QStringLiteral("/org/freedesktop/portal/desktop/request/1_42/lamemail1"));
        qputenv("QT_NO_XDG_DESKTOP_PORTAL", "1");
        UrlCatcher catcher;
        QDesktopServices::setUrlHandler(QStringLiteral("file"), &catcher, "open");
        auto restore = qScopeGuard([] {
            QDesktopServices::unsetUrlHandler(QStringLiteral("file"));
            qunsetenv("QT_NO_XDG_DESKTOP_PORTAL");
        });
        Opener opener;
        QSignalSpy failed(&opener, &Opener::failed);
        const QUrl url = QUrl::fromLocalFile(m_fake.filePath(QStringLiteral("out")));
        opener.open(url);
        QCOMPARE(catcher.urls, QList<QUrl> { url });
        QVERIFY(failed.isEmpty());
    }

    void mailDownloadsAllAttachmentsIntoAFolder()
    {
        Mail mail;
        const QString path = QStringLiteral("private/inbox/x-1.json");
        answer(R"({"subject":"Offer"})"
               "\n");
        mail.open(path);
        QVERIFY(QTest::qWaitFor([&] { return !mail.loading(); }));
        QSignalSpy downloaded(&mail, &Mail::attachmentsDownloaded);
        answer(
            R"({"index":0,"name":"offer.pdf","type":"application/pdf","size":13,"path":"/home/x/Downloads/offer.pdf"})"
            "\n"
            R"({"index":1,"name":"offer.pdf","type":"application/pdf","size":13,"path":"/home/x/Downloads/offer (1).pdf"})"
            "\n");
        const QString folder = QDir::homePath() + QStringLiteral("/Downloads");
        mail.downloadAttachments(folder);
        QVERIFY(mail.downloading());
        QTRY_COMPARE(downloaded.count(), 1);
        QVERIFY(!mail.downloading());
        QCOMPARE(downloaded.first().at(0).toInt(), 2);
        QCOMPARE(downloaded.first().at(1).toString(), QStringLiteral("~/Downloads"));
        QCOMPARE(read(m_fake.filePath(QStringLiteral("args"))),
            "--dir\n" + lamemail::dir().toUtf8() + "\nattachment\n--json\n--to=" + folder.toUtf8()
                + "\n--\n" + path.toUtf8() + "\n");
    }

    void senderNotesTheMailAndHandsBackTheJob()
    {
        Sender sender;
        QSignalSpy finished(&sender, &Sender::finished);
        answer(
            R"({"action":"send","id":3,"account":"private","path":"private/inbox/x-1.json","message_id":"a@example.com","to":[{"name":"Jane Doe","address":"jane@example.com"}],"subject":"Re: Offer","body":"Danke, Jane.\n"})"
            "\n");
        const auto reply = [](const QString& text) {
            return QVariantMap {
                { QStringLiteral("path"), QStringLiteral("private/inbox/x-1.json") },
                { QStringLiteral("to"),
                    QStringList { QStringLiteral("Jane Doe <jane@example.com>") } },
                { QStringLiteral("cc"), QStringList { QStringLiteral("paul@example.com") } },
                { QStringLiteral("subject"), QStringLiteral("Re: Offer") },
                { QStringLiteral("text"), text },
            };
        };
        sender.send(reply(QStringLiteral("Danke, Jane.")));
        QCOMPARE(sender.noting(), 1);
        QTRY_COMPARE(finished.count(), 1);
        QCOMPARE(sender.noting(), 0);
        QCOMPARE(read(m_fake.filePath(QStringLiteral("in"))), QByteArray("Danke, Jane."));
        QCOMPARE(read(m_fake.filePath(QStringLiteral("args"))),
            "--dir\n" + lamemail::dir().toUtf8()
                + "\nsend\n--json\n--subject=Re: Offer\n--reply=private/inbox/x-1.json\n"
                  "--to=Jane Doe <jane@example.com>\n--cc=paul@example.com\n");
        QVariantList noted = finished.takeFirst();
        QCOMPARE(noted.at(0).toMap().value(QStringLiteral("text")).toString(),
            QStringLiteral("Danke, Jane."));
        QCOMPARE(noted.at(1).toString(), QStringLiteral("a@example.com"));
        QVERIFY(noted.at(2).toString().isEmpty());

        answer({ }, 1, "lamemail: no SMTP server for `private`\n");
        sender.send(reply(QStringLiteral("Hallo")));
        QTRY_COMPARE(finished.count(), 1);
        QVERIFY(!read(m_fake.filePath(QStringLiteral("args"))).contains("--from"));
        noted = finished.takeFirst();
        QVERIFY(noted.at(1).toString().isEmpty());
        QCOMPARE(noted.at(2).toString(), QStringLiteral("no SMTP server for `private`"));

        answer(R"({"action":"send","id":5,"account":"private","message_id":"f@example.com"})"
               "\n");
        QVariantMap forward = reply(QStringLiteral("For you."));
        forward.insert(QStringLiteral("forward"), true);
        sender.send(forward);
        QTRY_COMPARE(finished.count(), 1);
        QVERIFY(read(m_fake.filePath(QStringLiteral("args")))
                .contains("\n--forward=private/inbox/x-1.json\n"));
        finished.clear();
        answer({ });
    }

    void foldingHidesTheQuotedLinesOfAQuote()
    {
        QQmlEngine engine;
        QQmlComponent component(&engine);
        component.setData(
            "import QtQuick\nTextEdit { text: \"Hi\\n\\nJean a écrit :\\n\\n> one\\n  > > "
            "two\\n\\n> three\\nBye\\n\\n> alone\\n> more\\n\\n\\n> far\" }",
            QUrl());
        std::unique_ptr<QObject> edit(component.create());
        QVERIFY(edit);
        Folding folding;
        folding.setDocument(edit->property("textDocument").value<QQuickTextDocument*>());
        QTextDocument* document = folding.document()->textDocument();
        QCOMPARE(folding.folds().size(), 2);
        QVariantMap fold = folding.folds().first().toMap();
        const int header = document->findBlockByNumber(2).position();
        QCOMPARE(fold.value(QStringLiteral("position")).toInt(), header);
        QCOMPARE(fold.value(QStringLiteral("lines")).toInt(), 5);
        QVERIFY(!fold.value(QStringLiteral("folded")).toBool());
        const QVariantMap alone = folding.folds().last().toMap();
        QCOMPARE(alone.value(QStringLiteral("position")).toInt(),
            document->findBlockByNumber(10).position());
        QCOMPARE(alone.value(QStringLiteral("lines")).toInt(), 1);

        folding.fold(header);
        QVERIFY(folding.folds().first().toMap().value(QStringLiteral("folded")).toBool());
        for (int number = 3; number <= 7; ++number)
            QVERIFY(!document->findBlockByNumber(number).isVisible());
        QVERIFY(document->findBlockByNumber(8).isVisible());
        QVERIFY(edit->property("text").toString().contains(QStringLiteral("> > two")));

        folding.reveal(document->findBlockByNumber(5).position());
        QVERIFY(document->findBlockByNumber(4).isVisible());
        folding.foldAll();
        QVERIFY(!document->findBlockByNumber(11).isVisible());
        folding.toggle(header);
        QVERIFY(!folding.folds().first().toMap().value(QStringLiteral("folded")).toBool());

        const QString before = edit->property("text").toString();
        folding.replace(QStringLiteral("Hello\n\nAm Montag schrieb Jean:\n> one\n> two"));
        QCOMPARE(edit->property("text").toString(),
            QStringLiteral("Hello\n\nAm Montag schrieb Jean:\n> one\n> two"));
        QCOMPARE(folding.folds().size(), 1);
        QVERIFY(folding.folds().first().toMap().value(QStringLiteral("folded")).toBool());
        document->undo();
        QCOMPARE(edit->property("text").toString(), before);
    }

    void localFilesAreDescribedWithNameAndSize()
    {
        QTemporaryDir dir;
        const QString plan = dir.filePath(QStringLiteral("plan.csv"));
        write(plan, "a,b\n");
        const QVariantList files = LocalFiles().describe({ QUrl::fromLocalFile(plan),
            QUrl::fromLocalFile(dir.path()), QUrl(QStringLiteral("https://example.com/x.pdf")),
            QUrl::fromLocalFile(dir.filePath(QStringLiteral("gone.pdf"))) });
        QCOMPARE(files.size(), 1);
        const QVariantMap file = files.first().toMap();
        QCOMPARE(file.value(QStringLiteral("path")).toString(), plan);
        QCOMPARE(file.value(QStringLiteral("name")).toString(), QStringLiteral("plan.csv"));
        QCOMPARE(file.value(QStringLiteral("size")).toString(), QStringLiteral("4 B"));
    }

    void senderWritesANewMailWithItsPeople()
    {
        Sender sender;
        QSignalSpy finished(&sender, &Sender::finished);
        answer(R"({"action":"send","id":4,"account":"work","message_id":"b@example.com"})"
               "\n");
        sender.send({
            { QStringLiteral("account"), QStringLiteral("work") },
            { QStringLiteral("from"), QStringLiteral("Shop <shop@example.com>") },
            { QStringLiteral("to"),
                QStringList { QStringLiteral("Jane Doe <jane@example.com>"),
                    QStringLiteral("tom@example.com") } },
            { QStringLiteral("cc"), QStringList { QStringLiteral("paul@example.com") } },
            { QStringLiteral("bcc"), QStringList { QStringLiteral("anna@example.com") } },
            { QStringLiteral("subject"), QStringLiteral("-Lunch") },
            { QStringLiteral("text"), QStringLiteral("Tuesday?") },
            { QStringLiteral("references"),
                QStringList { QStringLiteral("0"), QStringLiteral("work/inbox/x-1.json:2") } },
            { QStringLiteral("files"), QStringList { QStringLiteral("/home/me/plan.pdf") } },
        });
        QTRY_COMPARE(finished.count(), 1);
        QCOMPARE(read(m_fake.filePath(QStringLiteral("in"))), QByteArray("Tuesday?"));
        QCOMPARE(read(m_fake.filePath(QStringLiteral("args"))),
            "--dir\n" + lamemail::dir().toUtf8()
                + "\nsend\n--json\n--subject=-Lunch\n--from=Shop <shop@example.com>\n"
                  "--to=Jane Doe <jane@example.com>\n--to=tom@example.com\n--cc=paul@example.com\n"
                  "--bcc=anna@example.com\n--attachment=0\n--attachment=work/inbox/x-1.json:2\n"
                  "--file=/home/me/plan.pdf\n--\nwork\n");
        const QVariantList noted = finished.takeFirst();
        QCOMPARE(noted.at(0).toMap().value(QStringLiteral("account")).toString(),
            QStringLiteral("work"));
        QCOMPARE(noted.at(1).toString(), QStringLiteral("b@example.com"));
        answer({ });
    }

    void senderSavesADraftAndContinuesIt()
    {
        Sender sender;
        QSignalSpy saved(&sender, &Sender::saved);
        QSignalSpy finished(&sender, &Sender::finished);
        answer(R"({"action":"draft","id":6,"account":"work","message_id":"d@example.com"})"
               "\n");
        const QVariantMap job {
            { QStringLiteral("account"), QStringLiteral("work") },
            { QStringLiteral("subject"), QStringLiteral("Plan") },
            { QStringLiteral("text"), QStringLiteral("First thoughts") },
            { QStringLiteral("draftId"), QStringLiteral("d@example.com") },
        };
        sender.save(job);
        QCOMPARE(sender.noting(), 1);
        QTRY_COMPARE(saved.count(), 1);
        QCOMPARE(finished.count(), 0);
        QCOMPARE(read(m_fake.filePath(QStringLiteral("args"))),
            "--dir\n" + lamemail::dir().toUtf8()
                + "\ndraft\n--json\n--subject=Plan\n--draft=d@example.com\n--\nwork\n");
        QCOMPARE(saved.takeFirst().at(1).toString(), QStringLiteral("d@example.com"));
        sender.send(job);
        QTRY_COMPARE(finished.count(), 1);
        QVERIFY(read(m_fake.filePath(QStringLiteral("args")))
                .contains("\nsend\n--json\n--subject=Plan\n--draft=d@example.com\n"));

        answer({ }, 1, "lamemail: account `work` has no drafts folder\n");
        sender.save(job);
        QTRY_COMPARE(saved.count(), 1);
        QCOMPARE(finished.count(), 1);
        const QVariantList failed = saved.takeFirst();
        QVERIFY(failed.at(1).toString().isEmpty());
        QCOMPARE(failed.at(2).toString(), QStringLiteral("account `work` has no drafts folder"));

        answer({ });
    }

    void senderDiscardsADraft()
    {
        Sender sender;
        QSignalSpy discarded(&sender, &Sender::discarded);
        const QVariantMap job {
            { QStringLiteral("account"), QStringLiteral("work") },
            { QStringLiteral("draftId"), QStringLiteral("d@example.com") },
        };
        answer("draft d@example.com discarded\n");
        sender.discard(job);
        QCOMPARE(sender.noting(), 1);
        QTRY_COMPARE(discarded.count(), 1);
        QCOMPARE(sender.noting(), 0);
        QCOMPARE(read(m_fake.filePath(QStringLiteral("args"))),
            "--dir\n" + lamemail::dir().toUtf8() + "\ndiscard\n--\nwork\nd@example.com\n");
        QVariantList done = discarded.takeFirst();
        QCOMPARE(done.at(0).toMap().value(QStringLiteral("draftId")).toString(),
            QStringLiteral("d@example.com"));
        QVERIFY(done.at(1).toString().isEmpty());

        answer({ }, 1, "lamemail: unknown account `work`\n");
        sender.discard(job);
        QTRY_COMPARE(discarded.count(), 1);
        done = discarded.takeFirst();
        QCOMPARE(done.at(1).toString(), QStringLiteral("unknown account `work`"));
        answer({ });
    }

    void senderDeletesAMailInTheTrashForGood()
    {
        Sender sender;
        QSignalSpy deleted(&sender, &Sender::deletedForGood);
        const QString path = QStringLiteral("work/trash/a-1.json");
        answer("work/trash/a-1.json: deleted\n");
        sender.deleteForGood(path);
        QCOMPARE(sender.noting(), 1);
        QTRY_COMPARE(deleted.count(), 1);
        QCOMPARE(sender.noting(), 0);
        QCOMPARE(read(m_fake.filePath(QStringLiteral("args"))),
            "--dir\n" + lamemail::dir().toUtf8() + "\ndelete\n--\nwork/trash/a-1.json\n");
        QVariantList done = deleted.takeFirst();
        QCOMPARE(done.at(0).toString(), path);
        QVERIFY(done.at(1).toString().isEmpty());

        answer({ }, 1, "lamemail: `work/trash/a-1.json` is not in the trash\n");
        sender.deleteForGood(path);
        QTRY_COMPARE(deleted.count(), 1);
        done = deleted.takeFirst();
        QCOMPARE(
            done.at(1).toString(), QStringLiteral("`work/trash/a-1.json` is not in the trash"));
        answer({ });
    }

    void aiLinesParse()
    {
        const auto none = aiSetupFromJson(R"({"command":null,"programs":[]})");
        QVERIFY(none && none->command.isEmpty() && none->programs.isEmpty());
        const auto set = aiSetupFromJson(
            R"({"active":"ollama","command":null,"ollama":"qwen3","custom":"llm","programs":[{"program":"ollama","name":"ollama · qwen3","url":"https:\/\/ollama.com","found":true,"running":true,"command":null,"ollama":"qwen3"},{"program":"claude-code","name":"Claude Code","url":"https:\/\/claude.com\/claude-code","found":false,"running":false,"command":null,"ollama":null}]})");
        QCOMPARE(set->active, QStringLiteral("ollama"));
        QCOMPARE(set->custom, QStringLiteral("llm"));
        QVERIFY(set->command.isEmpty());
        QCOMPARE(set->ollama, QStringLiteral("qwen3"));
        QCOMPARE(set->programs.size(), 2);
        const AiProgram& qwen = set->programs.first();
        QCOMPARE(qwen.program, QStringLiteral("ollama"));
        QCOMPARE(qwen.name, QStringLiteral("ollama · qwen3"));
        QCOMPARE(qwen.url, QStringLiteral("https://ollama.com"));
        QVERIFY(qwen.found && qwen.running && qwen.command.isEmpty());
        QCOMPARE(qwen.ollama, QStringLiteral("qwen3"));
        const AiProgram& claude = set->programs.last();
        QVERIFY(!claude.found && !claude.running && claude.ollama.isEmpty());
        QVERIFY(!aiSetupFromJson("[]"));
        const auto suggestion
            = suggestionFromJson(R"({"path":"work/inbox/a-1.json","text":"Hi Jane"})");
        QCOMPARE(suggestion->text, QStringLiteral("Hi Jane"));
        QVERIFY(!suggestionFromJson(R"({"path":"work/inbox/a-1.json"})"));
        const auto question
            = suggestionFromJson(R"({"path":"work/inbox/a-1.json","question":"Who instead?"})");
        QCOMPARE(question->question, QStringLiteral("Who instead?"));
        QVERIFY(question->text.isEmpty());
        QVERIFY(!question->context);
        const auto read = suggestionFromJson(
            R"({"path":"a","text":"Hi","context":[{"name":"rates.md","state":"cut"}],)"
            R"("searches":["weekend"],"found":[{"path":"p","subject":"S","date":"d"}],"earlier":[]})");
        QCOMPARE(read->searches, QStringList { QStringLiteral("weekend") });
        QCOMPARE(read->found.first().subject, QStringLiteral("S"));
        QVERIFY(read->earlier.isEmpty());
        QCOMPARE(read->context->size(), 1);
        QCOMPARE(read->context->first().name, QStringLiteral("rates.md"));
        QCOMPARE(read->context->first().state, QStringLiteral("cut"));
        const auto account
            = accountFromJson(R"({"id":"work","context_folder":"/home/me/context"})");
        QCOMPARE(account->contextFolder, QStringLiteral("/home/me/context"));
    }

    void aiReadsChoosesAndRemovesWhatAnswers()
    {
        Ai ai;
        QSignalSpy changed(&ai, &Ai::changed);
        const QString log = m_fake.filePath(QStringLiteral("log"));
        write(log, { });
        answer(
            R"({"active":null,"command":null,"ollama":null,"custom":null,"programs":[{"program":"claude-code","name":"Claude Code","url":"https:\/\/claude.com/claude-code","found":true,"running":true,"command":"claude -p","ollama":null}]})"
            "\n");
        ai.reload();
        QTRY_COMPARE(changed.count(), 1);
        QVERIFY(!ai.ready());
        QCOMPARE(ai.programs().first().toMap().value(QStringLiteral("command")).toString(),
            QStringLiteral("claude -p"));

        answer(
            R"({"active":"claude-code","command":"claude -p","ollama":null,"custom":null,"programs":[]})"
            "\n");
        ai.choose(QStringLiteral("claude-code"), { });
        QTRY_COMPARE(changed.count(), 2);
        QVERIFY(ai.ready());
        QCOMPARE(ai.active(), QStringLiteral("claude-code"));
        QVERIFY(read(log).contains("\nai\nset\nclaude-code\nstdin:"));

        answer(R"({"active":"custom","command":"cat","ollama":null,"custom":"cat","programs":[]})"
               "\n");
        ai.choose(QStringLiteral("custom"), QStringLiteral(" cat "));
        QTRY_COMPARE(ai.custom(), QStringLiteral("cat"));
        QCOMPARE(ai.active(), QStringLiteral("custom"));
        QVERIFY(read(log).contains("\nai\nset\ncustom\n--\ncat\n"));

        answer(
            R"({"active":"custom","command":"cat","ollama":null,"custom":"cat","programs":[],"context_limit":50000,"context_limits":[10000,20000,50000,100000]})"
            "\n");
        ai.setContextLimit(50000);
        QTRY_COMPARE(ai.contextLimit(), 50000);
        QCOMPARE(ai.contextLimits().size(), 4);
        QCOMPARE(ai.contextLimits().last().toInt(), 100000);
        QVERIFY(read(log).contains("\nai\ncontext-limit\n50000\n"));

        answer(
            R"({"active":"ollama","command":null,"ollama":"gemma4:12b","custom":"cat","programs":[]})"
            "\n");
        ai.choose(QStringLiteral("ollama"), QStringLiteral("gemma4:12b"));
        QTRY_COMPARE(ai.ollama(), QStringLiteral("gemma4:12b"));
        QVERIFY(ai.ready() && ai.command().isEmpty() && ai.custom() == QStringLiteral("cat"));
        QVERIFY(read(log).contains("\nai\nset\nollama\n--\ngemma4:12b\n"));

        answer({ }, 1, "lamemail: the AI program `nope` is not found\n");
        ai.choose(QStringLiteral("custom"), QStringLiteral("nope"));
        QTRY_COMPARE(ai.error(), QStringLiteral("the AI program `nope` is not found"));
        answer(R"({"active":null,"command":null,"ollama":null,"custom":"cat","programs":[]})"
               "\n");
        ai.remove();
        QTRY_VERIFY(!ai.ready());
        QVERIFY(ai.error().isEmpty() && ai.custom() == QStringLiteral("cat"));
        QVERIFY(read(log).contains("\nai\nremove\n"));
        answer({ });
    }

    void aiReportSumsUpAndListsWhatTheAiGot()
    {
        const QDateTime now = QDateTime::currentDateTime();
        Suggestion none;
        const QVariantMap bare = aiReport(none, now, Lang::En);
        QCOMPARE(
            bare.value(QStringLiteral("summary")).toString(), QStringLiteral("No context folder"));
        const QVariantList bareSections = bare.value(QStringLiteral("sections")).toList();
        QCOMPARE(bareSections.size(), 1);
        QCOMPARE(bareSections.first().toMap().value(QStringLiteral("lines")).toStringList(),
            QStringList {
                QStringLiteral("None is set for this account, under Settings → AI → Context") });

        Suggestion full;
        full.context = QList<ContextEntry> {
            { .name = QStringLiteral("profile.md"), .state = QStringLiteral("read") },
            { .name = QStringLiteral("rates.md"), .state = QStringLiteral("cut") },
            { .name = QStringLiteral("offer.pdf"), .state = QStringLiteral("not_text") },
        };
        full.searches = { QStringLiteral("weekend"), QStringLiteral("rates") };
        full.found = { { .path = QStringLiteral("a"),
            .subject = QStringLiteral("Weekend table"),
            .date = QStringLiteral("2026-09-15T14:32:00+02:00") } };
        full.earlier
            = { { .path = QStringLiteral("b"), .subject = { }, .date = QStringLiteral("x") },
                  { .path = QStringLiteral("c"),
                      .subject = QStringLiteral("Project"),
                      .date = QStringLiteral("y") } };
        const QVariantMap report = aiReport(full, now, Lang::De);
        QCOMPARE(report.value(QStringLiteral("summary")).toString(),
            QStringLiteral("Kontext-Dateien: 2 von 3  ·  Suchtreffer: 1  ·  Frühere Mails: 2"));
        const QVariantList sections = report.value(QStringLiteral("sections")).toList();
        QCOMPARE(sections.size(), 3);
        QCOMPARE(sections[0].toMap().value(QStringLiteral("lines")).toStringList(),
            (QStringList { QStringLiteral("profile.md"), QStringLiteral("rates.md (gekürzt)"),
                QStringLiteral("offer.pdf (übersprungen: kein Text)") }));
        QCOMPARE(sections[1].toMap().value(QStringLiteral("title")).toString(),
            QStringLiteral("Suche: weekend, rates"));
        QVERIFY(sections[1]
                .toMap()
                .value(QStringLiteral("lines"))
                .toStringList()
                .first()
                .startsWith(QStringLiteral("Weekend table  ·  ")));
        QCOMPARE(sections[2].toMap().value(QStringLiteral("lines")).toStringList(),
            (QStringList { QStringLiteral("—  ·  x"), QStringLiteral("Project  ·  y") }));
    }

    void suggesterDraftsAReplyAndStopsWhenCancelled()
    {
        Suggester suggester;
        QSignalSpy suggested(&suggester, &Suggester::suggested);
        QSignalSpy failed(&suggester, &Suggester::failed);
        QSignalSpy reported(&suggester, &Suggester::reported);
        answer(R"({"path":"work/inbox/a-1.json","text":"Hallo Jane,\n\ngern."})"
               "\n");
        suggester.suggest(QStringLiteral("work/inbox/a-1.json"), QStringLiteral("Sage zu"),
            { QStringLiteral("Jane <jane@example.com>") }, { QStringLiteral("tom@example.com") });
        QVERIFY(suggester.running());
        QTRY_COMPARE(suggested.count(), 1);
        QVERIFY(!suggester.running());
        QCOMPARE(suggested.first().first().toString(), QStringLiteral("Hallo Jane,\n\ngern."));
        QCOMPARE(reported.count(), 1);
        QCOMPARE(read(m_fake.filePath(QStringLiteral("in"))), QByteArray("Sage zu"));
        QCOMPARE(read(m_fake.filePath(QStringLiteral("args"))),
            "--dir\n" + lamemail::dir().toUtf8()
                + "\nai\nsuggest\n--json\n--to=Jane <jane@example.com>\n--cc=tom@example.com\n"
                  "--before=\n--after=\n--\nwork/inbox/a-1.json\n");

        suggester.suggest(QStringLiteral("work/inbox/a-1.json"), QStringLiteral("Urlaub"), { }, { },
            QStringLiteral("Hallo Jane,\n\n"), QStringLiteral("\n\nRobin"));
        QTRY_COMPARE(suggested.count(), 2);
        QCOMPARE(read(m_fake.filePath(QStringLiteral("args"))),
            "--dir\n" + lamemail::dir().toUtf8()
                + "\nai\nsuggest\n--json\n--before=Hallo Jane,\n\n\n--after=\n\nRobin\n--\n"
                  "work/inbox/a-1.json\n");

        QSignalSpy asked(&suggester, &Suggester::asked);
        answer(R"({"path":"work/inbox/a-1.json","question":"Wen statt dir?"})"
               "\n");
        suggester.suggest(QStringLiteral("work/inbox/a-1.json"), QStringLiteral("Absagen, Tom"),
            { }, { }, { }, { }, true);
        QTRY_COMPARE(asked.count(), 1);
        QCOMPARE(asked.first().first().toString(), QStringLiteral("Wen statt dir?"));
        QCOMPARE(suggested.count(), 2);
        QVERIFY(read(m_fake.filePath(QStringLiteral("args"))).contains("--no-questions\n"));

        answer({ }, 1, "lamemail: no AI is set up\n");
        suggester.suggest(QStringLiteral("work/inbox/a-1.json"), { }, { }, { });
        QTRY_COMPARE(failed.count(), 1);
        QCOMPARE(failed.first().first().toString(), QStringLiteral("no AI is set up"));

        const QString pause = m_fake.filePath(QStringLiteral("sleep-ai"));
        write(pause, "30");
        auto removePause = qScopeGuard([&] { QFile::remove(pause); });
        suggester.suggest(QStringLiteral("work/inbox/a-1.json"), { }, { }, { });
        QTest::qWait(200);
        suggester.cancel();
        QVERIFY(!suggester.running());
        QTest::qWait(300);
        QCOMPARE(suggested.count(), 2);
        QCOMPARE(failed.count(), 1);
        answer({ });
    }

    void contactsListEveryoneAndFilterBySearch()
    {
        Contacts contacts;
        QAbstractItemModelTester tester(&contacts);
        answer(
            R"({"name":"Paul Berger","address":"paul@example.com","account":"work","color":"green","last":"2026-09-15T14:32:05+02:00","written":true,"later":1})"
            "\n"
            R"({"name":null,"address":"Maria.Vogel@example.com","account":"private","color":"blue","last":"2026-09-14T09:00:00+02:00","written":false})"
            "\n"
            R"({"name":"broken"})"
            "\n");
        contacts.reload();
        QVERIFY(QTest::qWaitFor([&] { return !contacts.loading(); }));
        QCOMPARE(read(m_fake.filePath(QStringLiteral("args"))),
            "--dir\n" + lamemail::dir().toUtf8() + "\ncontacts\n--json\n");
        QCOMPARE(contacts.rowCount(), 2);
        const QModelIndex first = contacts.index(0);
        QCOMPARE(
            contacts.data(first, Contacts::NameRole).toString(), QStringLiteral("Paul Berger"));
        QCOMPARE(
            contacts.data(first, Contacts::AccountColorRole).toString(), QStringLiteral("green"));
        QVERIFY(contacts.data(first, Contacts::WrittenRole).toBool());
        QVERIFY(!contacts.data(first, Contacts::TimeRole).toString().isEmpty());
        QVERIFY(contacts.data(contacts.index(1), Contacts::NameRole).toString().isEmpty());

        contacts.setQuery(QStringLiteral("vogel MARIA"));
        QCOMPARE(contacts.rowCount(), 1);
        QCOMPARE(contacts.data(contacts.index(0), Contacts::AddressRole).toString(),
            QStringLiteral("Maria.Vogel@example.com"));
        contacts.setQuery(QStringLiteral("berger nobody"));
        QCOMPARE(contacts.rowCount(), 0);
        contacts.setQuery({ });
        QCOMPARE(contacts.rowCount(), 2);

        const auto address = [&](int row) {
            return contacts.data(contacts.index(row), Contacts::AddressRole).toString();
        };
        contacts.setChosen({ QStringLiteral("maria.vogel@EXAMPLE.com") });
        QCOMPARE(address(0), QStringLiteral("paul@example.com"));
        contacts.arrange();
        QCOMPARE(address(0), QStringLiteral("Maria.Vogel@example.com"));
        contacts.setQuery(QStringLiteral("example"));
        QCOMPARE(address(0), QStringLiteral("Maria.Vogel@example.com"));
        contacts.setChosen(
            { QStringLiteral("paul@example.com"), QStringLiteral("maria.vogel@example.com") });
        contacts.setQuery({ });
        QCOMPARE(address(0), QStringLiteral("paul@example.com"));
        QCOMPARE(address(1), QStringLiteral("Maria.Vogel@example.com"));
        contacts.setChosen({ });
        contacts.arrange();
        QCOMPARE(address(0), QStringLiteral("paul@example.com"));

        QCOMPARE(contacts.accountOf(QStringLiteral(" maria.vogel@EXAMPLE.com")),
            QStringLiteral("private"));
        QVERIFY(contacts.accountOf(QStringLiteral("stranger@example.com")).isEmpty());

        QSignalSpy resets(&contacts, &Contacts::modelReset);
        contacts.reload();
        QVERIFY(QTest::qWaitFor([&] { return !contacts.loading(); }));
        QCOMPARE(resets.count(), 0);

        answer({ }, 1, "lamemail: no settings\n");
        contacts.reload();
        QVERIFY(QTest::qWaitFor([&] { return !contacts.loading(); }));
        QCOMPARE(contacts.error(), QStringLiteral("no settings"));
        QCOMPARE(contacts.rowCount(), 2);
        answer({ });
    }

    void accountsListAddAndReportPerAccount()
    {
        Accounts accounts;
        QAbstractItemModelTester tester(&accounts);
        const QByteArray list
            = R"({"id":"private","host":"imap.example.com","port":993,"username":"me@example.com","smtp":{"host":"smtp.example.com","port":465,"copy_to_sent":false},"color":"green","signature":"Me\nexample.com","archive_by_year":true,"from":{"name":"Me","address":"me@example.com"},"password":"missing","kinds":{"inbox":"inbox","archive":null,"trash":"bin"},"folders":{"inbox":{"kind":"inbox","sync":true,"default":true},"notes":{"kind":"other","sync":false}}})"
              "\n"
              R"({"id":"work","host":"imap.example.com","port":993,"username":"work@example.com","smtp":null,"password":"stored"})"
              "\n";
        answer(list);
        accounts.reload();
        QVERIFY(QTest::qWaitFor([&] { return !accounts.loading(); }));
        QCOMPARE(accounts.rowCount(), 2);
        const QModelIndex first = accounts.index(0);
        QCOMPARE(accounts.data(first, Accounts::IdRole).toString(), QStringLiteral("private"));
        QCOMPARE(accounts.data(first, Accounts::SmtpPortRole).toInt(), 465);
        QVERIFY(!accounts.data(first, Accounts::CopyToSentRole).toBool());
        QVERIFY(accounts.data(first, Accounts::ArchiveByYearRole).toBool());
        QVERIFY(!accounts.data(accounts.index(1), Accounts::ArchiveByYearRole).toBool());
        QCOMPARE(accounts.data(first, Accounts::ColorRole).toString(), QStringLiteral("green"));
        QCOMPARE(accounts.data(first, Accounts::SignatureRole).toString(),
            QStringLiteral("Me\nexample.com"));
        QCOMPARE(
            accounts.data(first, Accounts::PasswordRole).toString(), QStringLiteral("missing"));
        const QVariantMap kinds = accounts.data(first, Accounts::KindsRole).toMap();
        QCOMPARE(kinds.value(QStringLiteral("trash")), QVariant(QStringLiteral("bin")));
        QVERIFY(kinds.value(QStringLiteral("archive")).isNull());
        const QVariantList folders = accounts.data(first, Accounts::FoldersRole).toList();
        QCOMPARE(folders.size(), 2);
        const QVariantMap inbox = folders.first().toMap();
        QCOMPARE(inbox.value(QStringLiteral("name")), QVariant(QStringLiteral("inbox")));
        QCOMPARE(inbox.value(QStringLiteral("kind")), QVariant(QStringLiteral("inbox")));
        QVERIFY(inbox.value(QStringLiteral("sync")).toBool());
        const QVariantMap notes = folders.last().toMap();
        QCOMPARE(notes.value(QStringLiteral("kind")), QVariant(QStringLiteral("other")));
        QVERIFY(!notes.value(QStringLiteral("sync")).toBool());
        QVERIFY(accounts.data(accounts.index(1), Accounts::SmtpHostRole).toString().isEmpty());
        QCOMPARE(
            accounts.ids(), QStringList({ QStringLiteral("private"), QStringLiteral("work") }));
        QCOMPARE(accounts.senders().value(QStringLiteral("private")),
            QVariant(QStringLiteral("me@example.com")));
        QCOMPARE(
            accounts.colors().value(QStringLiteral("private")), QVariant(QStringLiteral("green")));
        QVERIFY(accounts.senders().value(QStringLiteral("work")).toString().isEmpty());
        QCOMPARE(accounts.signatures().value(QStringLiteral("private")),
            QVariant(QStringLiteral("Me\nexample.com")));
        QVERIFY(accounts.signatures().value(QStringLiteral("work")).toString().isEmpty());
        QSignalSpy changed(&accounts, &Accounts::changed);
        accounts.reload();
        QVERIFY(QTest::qWaitFor([&] { return !accounts.loading(); }));
        QCOMPARE(changed.count(), 0);

        QSignalSpy finished(&accounts, &Accounts::addFinished);
        write(m_fake.filePath(QStringLiteral("log")), { });
        answer("login ok, 3 folders\nadded account `new`\n");
        accounts.add(QStringLiteral("new"), QStringLiteral("imap.example.com"), { },
            QStringLiteral("new@example.com"), QStringLiteral("geheim"),
            QStringLiteral("smtp.example.com"), QStringLiteral("587"));
        QTRY_COMPARE(finished.count(), 1);
        QVERIFY(finished.first().at(0).toBool());
        QCOMPARE(finished.first().at(2).toString(), QStringLiteral("added account `new`"));
        QVERIFY(read(m_fake.filePath(QStringLiteral("log")))
                .contains(
                    "account\nadd\n--host\nimap.example.com\n--user\nnew@example.com\n"
                    "--smtp-host\nsmtp.example.com\n--smtp-port\n587\n--\nnew\nstdin:geheim\n"));
        QVERIFY(QTest::qWaitFor([&] { return !accounts.loading(); }));

        QSignalSpy reloaded(&accounts, &Accounts::loadingChanged);
        answer("`private`: the tag is orange now\n");
        accounts.setColor(QStringLiteral("private"), QStringLiteral("orange"));
        QTRY_COMPARE(reloaded.count(), 2);
        QVERIFY(read(m_fake.filePath(QStringLiteral("log")))
                .contains("account\ncolor\nprivate\norange\n"));

        reloaded.clear();
        QSignalSpy assigned(&accounts, &Accounts::folderAssigned);
        answer("`private`: `notes` is archive, synchronized\n");
        accounts.assignFolder(QStringLiteral("private"), QStringLiteral("notes"),
            QStringLiteral("archive"), { }, false);
        QTRY_COMPARE(reloaded.count(), 2);
        QCOMPARE(assigned.count(), 1);
        QVERIFY(read(m_fake.filePath(QStringLiteral("log")))
                .contains("account\nfolder\n--\nprivate\nnotes\narchive\n"));

        reloaded.clear();
        answer("`private`: `notes` is archive, not synchronized, the default of its kind\n");
        accounts.assignFolder(QStringLiteral("private"), QStringLiteral("notes"),
            QStringLiteral("archive"), false, true);
        QTRY_COMPARE(reloaded.count(), 2);
        QCOMPARE(assigned.count(), 1);
        QVERIFY(read(m_fake.filePath(QStringLiteral("log")))
                .contains("account\nfolder\n--sync\nfalse\n--default\n--\nprivate\nnotes\n"
                          "archive\n"));

        reloaded.clear();
        answer({ }, 1, "lamemail: there is no folder `gone`\n");
        accounts.assignFolder(
            QStringLiteral("private"), QStringLiteral("gone"), QStringLiteral("trash"), { }, false);
        QTRY_COMPARE(reloaded.count(), 2);
        QCOMPARE(assigned.count(), 1);

        reloaded.clear();
        answer("`private`: archived mail goes into the archive itself\n");
        accounts.setArchiveByYear(QStringLiteral("private"), false);
        QTRY_COMPARE(reloaded.count(), 2);
        QVERIFY(read(m_fake.filePath(QStringLiteral("log")))
                .contains("account\narchive\nprivate\n--by-year\nfalse\n"));

        reloaded.clear();
        answer("`private`: signature saved\n");
        accounts.setSignature(QStringLiteral("private"), QStringLiteral("Me\nTel. 1"));
        QTRY_COMPARE(reloaded.count(), 2);
        QVERIFY(read(m_fake.filePath(QStringLiteral("log")))
                .contains("account\nsignature\nprivate\nstdin:Me\nTel. 1\n"));

        reloaded.clear();
        answer("`private`: the AI gets the text files of /home/me/context\n");
        accounts.setContextFolder(
            QStringLiteral("private"), QUrl::fromLocalFile(QStringLiteral("/home/me/context")));
        QTRY_COMPARE(reloaded.count(), 2);
        QVERIFY(read(m_fake.filePath(QStringLiteral("log")))
                .contains("account\ncontext\nprivate\n/home/me/context\n"));

        reloaded.clear();
        answer("`private`: no context for the AI\n");
        accounts.setContextFolder(QStringLiteral("private"), { });
        QTRY_COMPARE(reloaded.count(), 2);
        QVERIFY(read(m_fake.filePath(QStringLiteral("log")))
                .contains("account\ncontext\nprivate\nstdin:"));

        QSignalSpy stored(&accounts, &Accounts::passwordStored);
        answer("updated password for `private`\n");
        accounts.setPassword(QStringLiteral("private"), QStringLiteral("neu"));
        QTRY_COMPARE(stored.count(), 1);
        QCOMPARE(stored.first().at(0).toString(), QStringLiteral("private"));
        QVERIFY(QTest::qWaitFor([&] { return !accounts.loading(); }));

        answer(list);
        accounts.reload();
        QVERIFY(QTest::qWaitFor([&] { return !accounts.loading(); }));
        answer({ }, 1, "lamemail: login as me@example.com failed: wrong password\n");
        accounts.test(QStringLiteral("private"));
        QVERIFY(accounts.data(first, Accounts::BusyRole).toBool());
        QTRY_VERIFY(!accounts.data(accounts.index(0), Accounts::BusyRole).toBool());
        QCOMPARE(accounts.data(accounts.index(0), Accounts::StatusRole).toString(),
            QStringLiteral("login as me@example.com failed: wrong password"));
        QVERIFY(!accounts.data(accounts.index(0), Accounts::StatusOkRole).toBool());
        QVERIFY(QTest::qWaitFor([&] { return !accounts.loading(); }));
        answer({ });
    }

    void manualListsTheCommandsOfTheCli()
    {
        Manual manual;
        QSignalSpy changed(&manual, &Manual::changed);
        answer(
            R"({"command":"","usage":"lamemail [--dir <dir>] <command>","about":"Mirrors IMAP mailboxes","arguments":[{"name":"--dir <dir>","help":"Sync folder"}]})"
            "\n"
            R"({"command":"flag","usage":"lamemail flag <path> <change>","about":"Mark a message","arguments":[{"name":"<path>","help":"Message file"},{"name":"<change>","help":"","values":["seen","unseen"]}]})"
            "\n");
        manual.load();
        QVERIFY(changed.wait());
        QCOMPARE(read(m_fake.filePath(QStringLiteral("args"))),
            "--dir\n" + lamemail::dir().toUtf8() + "\ncommands\n--json\n");
        QCOMPARE(manual.commands().size(), 2);
        const QVariantMap flag = manual.commands().at(1).toMap();
        QCOMPARE(flag.value(QStringLiteral("usage")).toString(),
            QStringLiteral("lamemail flag <path> <change>"));
        const QVariantMap change = flag.value(QStringLiteral("arguments")).toList().at(1).toMap();
        QCOMPARE(change.value(QStringLiteral("values")).toStringList(),
            QStringList({ QStringLiteral("seen"), QStringLiteral("unseen") }));
        QVERIFY(manual.error().isEmpty());

        answer({ }, 2, "lamemail: unrecognized subcommand 'commands'\n");
        manual.load();
        QVERIFY(changed.wait());
        QVERIFY(manual.commands().isEmpty());
        QCOMPARE(manual.error(), QStringLiteral("unrecognized subcommand 'commands'"));
    }

    void foldersKeepAFailedChangeVisible()
    {
        Folders folders;
        QAbstractItemModelTester tester(&folders);
        answer(folder("inbox", false) + folder("junk", false));
        folders.reload();
        QVERIFY(QTest::qWaitFor([&] { return !folders.loading(); }));
        QCOMPARE(folders.rowCount(), 2);
        QVERIFY(!folders.data(folders.index(0), Folders::AnySetRole).toBool());

        answer({ }, 1, "lamemail: invalid value `x` for dense; use true, false\n");
        folders.set(QStringLiteral("inbox"), QStringLiteral("dense"), QStringLiteral("x"));
        QVERIFY(QTest::qWaitFor([&] { return !folders.error().isEmpty(); }));
        QCOMPARE(folders.error(), QStringLiteral("invalid value `x` for dense; use true, false"));

        answer(folder("inbox", true) + folder("junk", false));
        folders.set(QStringLiteral("inbox"), QStringLiteral("dense"), QStringLiteral("true"));
        QVERIFY(QTest::qWaitFor([&] { return folders.error().isEmpty(); }));
        QVERIFY(QTest::qWaitFor([&] { return !folders.loading(); }));
        QVERIFY(folders.data(folders.index(0), Folders::DenseRole).toBool());
        QVERIFY(folders.data(folders.index(0), Folders::DenseSetRole).toBool());
        QCOMPARE(folders.data(folders.index(0), Folders::IndentRole).toInt(), 3);
        QVERIFY(!folders.data(folders.index(0), Folders::IndentSetRole).toBool());
    }

    void watcherSeesMailAndThemeButNotTheLock()
    {
        QTemporaryDir home;
        const QString theme = home.filePath(QStringLiteral(".local/state/omarchy/current/theme"));
        QVERIFY(QDir().mkpath(theme));
        write(theme + QStringLiteral("/colors.toml"), "accent = \"#112233\"\n");
        const QByteArray realHome = qgetenv("HOME");
        const auto restore = qScopeGuard([&] {
            qputenv("HOME", realHome);
            lamemail::setDir(m_fake.path());
        });
        qputenv("HOME", home.path().toUtf8());
        QCOMPARE(Theme().accent(), QColor(0x11, 0x22, 0x33));

        QTemporaryDir mail;
        QVERIFY(QDir().mkpath(mail.filePath(QStringLiteral("private/inbox"))));
        lamemail::setDir(mail.path());
        Watcher watcher;
        QSignalSpy changed(&watcher, &Watcher::changed);
        QSignalSpy themeChanged(&watcher, &Watcher::themeChanged);
        watcher.start();

        write(mail.filePath(QStringLiteral(".lock")), "");
        QTest::qWait(700);
        QCOMPARE(changed.count(), 0);

        write(mail.filePath(QStringLiteral("private/inbox/2026-09-15_1432-a-1.json")), "{}");
        QTRY_COMPARE(changed.count(), 1);
        QVERIFY(QDir().mkpath(mail.filePath(QStringLiteral("business/inbox"))));
        QTRY_COMPARE(changed.count(), 2);
        write(mail.filePath(QStringLiteral("business/inbox/2026-09-15_1432-b-2.json")), "{}");
        QTRY_COMPARE(changed.count(), 3);
        QCOMPARE(themeChanged.count(), 0);

        write(theme + QStringLiteral("/colors.toml"), "accent = \"#445566\"\n");
        QTRY_COMPARE(themeChanged.count(), 1);
        QCOMPARE(changed.count(), 3);
    }

    void lightAndDarkReplaceTheOmarchyTheme()
    {
        QTemporaryDir home;
        const QString theme = home.filePath(QStringLiteral(".local/state/omarchy/current/theme"));
        QVERIFY(QDir().mkpath(theme));
        write(theme + QStringLiteral("/colors.toml"), "accent = \"#112233\"\nmode = \"light\"\n");
        const QByteArray realHome = qgetenv("HOME");
        const auto restore = qScopeGuard([&] {
            qputenv("HOME", realHome);
            QCoreApplication::setOrganizationName({ });
        });
        qputenv("HOME", home.path().toUtf8());
        QSettings::setPath(
            QSettings::NativeFormat, QSettings::UserScope, home.filePath(QStringLiteral("config")));
        QCoreApplication::setOrganizationName(QStringLiteral("lamemail"));

        Preferences preferences;
        QSignalSpy changed(&preferences, &Preferences::themeChanged);
        QCOMPARE(Preferences::theme(), QStringLiteral("system"));
        QCOMPARE(Theme().accent(), QColor(0x11, 0x22, 0x33));
        QCOMPARE(Theme().background(), QColor(Qt::white));

        preferences.setTheme(QStringLiteral("dark"));
        QCOMPARE(changed.count(), 1);
        QCOMPARE(Theme().accent(), QColor(0x89, 0xb4, 0xfa));
        QCOMPARE(Theme().background(), QColor(0x1e, 0x1e, 0x2e));

        preferences.setTheme(QStringLiteral("light"));
        preferences.setTheme(QStringLiteral("light"));
        QCOMPARE(changed.count(), 2);
        QCOMPARE(Theme().accent(), QColor(0x1e, 0x66, 0xf5));
        QCOMPARE(Theme().background(), QColor(Qt::white));
    }

    void feedKindsKeepAtLeastOneAndStartWithInboxSentAndDrafts()
    {
        QTemporaryDir home;
        const auto restore = ownSettings(home);

        Preferences preferences;
        QSignalSpy changed(&preferences, &Preferences::feedKindsChanged);
        QCOMPARE(preferences.feedKinds(), QStringList({ "inbox", "sent", "drafts" }));
        preferences.setFeedKinds({ });
        preferences.setFeedKinds(
            { QStringLiteral("inbox"), QStringLiteral("sent"), QStringLiteral("drafts") });
        QCOMPARE(changed.count(), 0);
        preferences.setFeedKinds({ QStringLiteral("trash") });
        QCOMPARE(changed.count(), 1);
        QCOMPARE(Preferences().feedKinds(), QStringList({ "trash" }));
    }

    void shadowsStayOnUntilTurnedOff()
    {
        QTemporaryDir home;
        const auto restore = ownSettings(home);
        Preferences preferences;
        QSignalSpy changed(&preferences, &Preferences::shadowsChanged);
        QVERIFY(preferences.shadows());
        preferences.setShadows(true);
        QCOMPARE(changed.count(), 0);
        preferences.setShadows(false);
        QCOMPARE(changed.count(), 1);
        QVERIFY(!Preferences().shadows());
    }

    void instanceHandsOverToTheOneOfTheSameFolder()
    {
        QTemporaryDir runtime;
        const QByteArray realRuntime = qgetenv("XDG_RUNTIME_DIR");
        const auto restore = qScopeGuard([&] { qputenv("XDG_RUNTIME_DIR", realRuntime); });
        qputenv("XDG_RUNTIME_DIR", runtime.path().toUtf8());

        Instance first(QStringLiteral("mail"));
        if (!first.listen())
            QSKIP("no local sockets here");
        QSignalSpy activated(&first, &Instance::activated);
        QVERIFY(Instance(QStringLiteral("./mail")).handOver("token"));
        QVERIFY(activated.wait());
        QCOMPARE(activated.at(0).at(0).toByteArray(), QByteArray("token"));
        QVERIFY(!Instance(QStringLiteral("other")).handOver({ }));
    }
};

int main(int argc, char** argv)
{
    qputenv("QT_QPA_PLATFORM", "offscreen");
    qunsetenv("QT_QPA_PLATFORMTHEME");
    QGuiApplication app(argc, argv);
    TestLamemail test;
    return QTest::qExec(&test, argc, argv);
}

#include "tst_lamemail.moc"
