#include "watcher.h"

#include "lamemail.h"
#include "theme.h"

#include <QDebug>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <cerrno>
#include <cstring>
#include <sys/inotify.h>
#include <unistd.h>

namespace {

constexpr int quietMs = 400;
constexpr uint32_t mask
    = IN_CREATE | IN_DELETE | IN_MODIFY | IN_ATTRIB | IN_MOVED_FROM | IN_MOVED_TO | IN_DELETE_SELF;
const QString lockSuffix = QStringLiteral(".lock");

}

bool isInside(const QString& path, const QString& dir)
{
    return !dir.isEmpty() && (path == dir || path.startsWith(dir + QLatin1Char('/')));
}

Watcher::Watcher(QObject* parent)
    : QObject(parent)
{
    m_quiet.setSingleShot(true);
    m_quiet.setInterval(quietMs);
    connect(&m_quiet, &QTimer::timeout, this, [this] {
        const bool mail = std::exchange(m_mailHit, false);
        const bool theme = std::exchange(m_themeHit, false);
        if (mail)
            emit changed();
        if (theme)
            emit themeChanged();
    });
}

Watcher::~Watcher()
{
    delete m_notifier;
    if (m_fd >= 0)
        ::close(m_fd);
}

void Watcher::start()
{
    if (m_fd >= 0)
        return;
    m_fd = inotify_init1(IN_NONBLOCK | IN_CLOEXEC);
    if (m_fd < 0) {
        qWarning().noquote() << "lamemail-ui: watching:" << std::strerror(errno);
        return;
    }
    m_mail = QDir(lamemail::dir()).absolutePath();
    m_theme = omarchyCurrent();
    addTree(m_mail);
    if (!m_theme.isEmpty())
        addTree(m_theme);
    m_notifier = new QSocketNotifier(m_fd, QSocketNotifier::Read, this);
    connect(m_notifier, &QSocketNotifier::activated, this, &Watcher::read);
}

void Watcher::addTree(const QString& dir)
{
    const int wd = inotify_add_watch(m_fd, QFile::encodeName(dir).constData(), mask);
    if (wd < 0) {
        qWarning().noquote() << "lamemail-ui: watching" << dir << std::strerror(errno);
        return;
    }
    m_dirs.insert(wd, dir);
    const auto subdirs = QDir(dir).entryInfoList(
        QDir::Dirs | QDir::NoDotAndDotDot | QDir::Hidden | QDir::NoSymLinks);
    for (const QFileInfo& sub : subdirs)
        addTree(sub.filePath());
}

void Watcher::read()
{
    alignas(inotify_event) char buffer[16384];
    for (;;) {
        const ssize_t length = ::read(m_fd, buffer, sizeof buffer);
        if (length <= 0)
            break;
        for (const char* at = buffer; at < buffer + length;) {
            const auto* event = reinterpret_cast<const inotify_event*>(at);
            at += sizeof(inotify_event) + event->len;
            if (event->mask & IN_IGNORED) {
                m_dirs.remove(event->wd);
                continue;
            }
            const QString dir = m_dirs.value(event->wd);
            const QString name = event->len ? QFile::decodeName(event->name) : QString();
            const QString path = name.isEmpty() ? dir : dir + QLatin1Char('/') + name;
            if ((event->mask & IN_ISDIR) && (event->mask & (IN_CREATE | IN_MOVED_TO)))
                addTree(path);
            if (name.endsWith(lockSuffix))
                continue;
            if (isInside(path, m_theme))
                m_themeHit = true;
            else
                m_mailHit = true;
        }
    }
    if (m_mailHit || m_themeHit)
        m_quiet.start();
}
