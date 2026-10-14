#pragma once

#include <QDateTime>
#include <QObject>
#include <QString>
#include <QVariantList>
#include <QtQml/qqmlregistration.h>
#include <array>

enum class Lang { En, De };

struct DateLength {
    const char* name;
    const char* format;
};

struct Texts {
    const char* wordmark;
    const char* license;
    const char* licenseLink;
    const char* version;
    const char* infoSection;
    const char* infoHint;
    const char* sourceCode;
    const char* projectLink;
    const char* issues;
    const char* issuesLink;
    const char* licenseTitle;
    const char* thirdParty;
    const char* thirdPartyHint;
    const char* liabilityTitle;
    const char* liability;
    const char* unread;
    const char* hits;
    const char* empty;
    const char* noResults;
    const char* emptyKind;
    const char* kindOnly;
    const char* noAccounts;
    const char* commands;
    const char* exitedWithCode;
    const char* crashed;
    const char* openFailed;
    const char* quote;
    const char* to;
    const char* replyAll;
    const char* replyMode;
    const char* forwardMode;
    const char* modeTip;
    const char* menuCut;
    const char* menuCopy;
    const char* menuPaste;
    const char* menuSelectAll;
    const char* ctrlKey;
    const char* copied;
    const char* attachTip;
    const char* quoteCommand;
    const char* quoteCommandHint;
    const char* signatureCommand;
    const char* signatureCommandHint;
    const char* noSignature;
    const char* attachCommand;
    const char* dateCommand;
    const char* dateCommandHint;
    const char* aiCommand;
    const char* aiCommandHint;
    const char* aiCommandOff;
    const char* aiCommandReplyOnly;
    const char* aiThinking;
    const char* aiStop;
    const char* aiAsks;
    const char* aiAnswerPlaceholder;
    const char* aiSkip;
    const char* gapLeft;
    const char* aiReportFiles;
    const char* aiReportNoFolder;
    const char* aiReportHits;
    const char* aiReportEarlier;
    const char* aiReportFolderTitle;
    const char* aiReportNoFolderLine;
    const char* aiReportEmptyFolder;
    const char* aiReportSkipped;
    const char* aiReportSearchTitle;
    const char* aiReportNoHits;
    const char* aiReportEarlierTitle;
    const char* aiReportTip;
    const char* aiReportHide;
    const char* aiContextCut;
    const char* aiContextNotText;
    const char* aiContextEmptyFile;
    const char* aiContextLimit;
    const char* aiContextFolder;
    const char* foldedLines;
    const char* foldTip;
    const char* syncFolder;
    const char* replyPlaceholder;
    const char* sendTip;
    const char* saveDraftTip;
    const char* draftSaved;
    const char* discardTip;
    const char* discarding;
    const char* deleting;
    const char* sending;
    const char* sentTo;
    const char* undo;
    const char* undoKey;
    const char* upToDate;
    const char* archived;
    const char* trashed;
    const char* movedToInbox;
    const char* current;
    const char* lastSyncTip;
    const char* closingAfterSend;
    const char* noText;
    const char* newMail;
    const char* ccKind;
    const char* bccKind;
    const char* senderPlaceholder;
    const char* subject;
    const char* mailPlaceholder;
    const char* addressPlaceholder;
    const char* searchMail;
    const char* searchContacts;
    const char* noContacts;
    const char* noMatch;
    const char* pickHint;
    const char* shortcuts;
    const char* settings;
    const char* feedSection;
    const char* feedHint;
    const char* generalSection;
    const char* generalHint;
    const char* theme;
    const char* themeSystem;
    const char* themeLight;
    const char* themeDark;
    const char* wheel;
    const char* wheelHint;
    const char* closeToTray;
    const char* closeToTrayHint;
    const char* autostart;
    const char* flaggedFirst;
    const char* feedKinds;
    const char* feedKindsHint;
    const char* shadows;
    const char* shadowsMissing;
    const char* notifications;
    const char* notificationsHint;
    const char* downloadFolder;
    const char* downloadFolderHint;
    const char* downloadAll;
    const char* newMails;
    const char* noTray;
    const char* open;
    const char* quit;
    const char* accountsSection;
    const char* accountsHint;
    const char* commandsSection;
    const char* commandsHint;
    const char* test;
    const char* edit;
    const char* save;
    const char* cancel;
    const char* addAccount;
    const char* add;
    const char* accountName;
    const char* accountNameHint;
    const char* imapServer;
    const char* smtpServer;
    const char* smtpHint;
    const char* login;
    const char* password;
    const char* passwordHint;
    const char* passwordMissing;
    const char* noSmtp;
    const char* defaultPort;
    const char* copyToSentHint;
    const char* on;
    const char* off;
    const char* passwordUnknown;
    const char* sendingVia;
    const char* copyToSent;
    const char* archiveByYear;
    const char* archiveByYearHint;
    const char* color;
    const char* signature;
    const char* signatureHint;
    const char* context;
    const char* contextHint;
    const char* contextNone;
    const char* contextChoose;
    const char* contextLimit;
    const char* contextLimitHint;
    const char* aiSection;
    const char* aiHint;
    const char* aiModel;
    const char* aiPrivacyTitle;
    const char* aiPrivacy;
    const char* aiProgram;
    const char* aiProgramHint;
    const char* aiProgramPlaceholder;
    const char* aiNotFound;
    const char* aiNoModel;
    const char* aiNotRunning;
    const char* aiAboutClaude;
    const char* aiAboutOllama;
    const char* removeAccount;
    const char* removeHint;
    const char* purge;
    const char* remove;
    const char* defaultFolders;
    const char* defaultFoldersHint;
    const char* kindsMissing;
    const char* kindOther;
    const char* folderSync;
    const char* folderSyncTip;
    const char* defaultFolder;
    const char* defaultFolderTip;
    const char* backTip;
    const char* searchTip;
    const char* composeTip;
    const char* settingsTip;
    const char* syncTip;
    const char* starTip;
    const char* unstarTip;
    const char* unreadTip;
    const char* readTip;
    const char* archiveTip;
    const char* trashTip;
    const char* inboxTip;
    const char* deleteTip;
    std::array<const char*, 7> icons;
    std::array<const char*, 4> flags;
    const char* indented;
    const char* now;
    const char* minutesAgo;
    const char* yesterday;
    const char* starred;
    std::array<const char*, 3> days;
    std::array<const char*, 7> weekdays;
    std::array<const char*, 12> months;
    std::array<DateLength, 3> dateLengths;
};

Lang langFrom(const QString& locale);
Lang lang();
const Texts& texts(Lang lang);
const Texts& texts();

class Strings : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(QString wordmark READ wordmark CONSTANT)
    Q_PROPERTY(QString license READ license CONSTANT)
    Q_PROPERTY(QString licenseLink READ licenseLink CONSTANT)
    Q_PROPERTY(QString version READ version CONSTANT)
    Q_PROPERTY(QString infoSection READ infoSection CONSTANT)
    Q_PROPERTY(QString infoHint READ infoHint CONSTANT)
    Q_PROPERTY(QString sourceCode READ sourceCode CONSTANT)
    Q_PROPERTY(QString projectLink READ projectLink CONSTANT)
    Q_PROPERTY(QString issues READ issues CONSTANT)
    Q_PROPERTY(QString issuesLink READ issuesLink CONSTANT)
    Q_PROPERTY(QString licenseTitle READ licenseTitle CONSTANT)
    Q_PROPERTY(QString thirdParty READ thirdParty CONSTANT)
    Q_PROPERTY(QString thirdPartyHint READ thirdPartyHint CONSTANT)
    Q_PROPERTY(QString liabilityTitle READ liabilityTitle CONSTANT)
    Q_PROPERTY(QString liability READ liability CONSTANT)
    Q_PROPERTY(QString unread READ unread CONSTANT)
    Q_PROPERTY(QString hits READ hits CONSTANT)
    Q_PROPERTY(QString empty READ empty CONSTANT)
    Q_PROPERTY(QString noResults READ noResults CONSTANT)
    Q_PROPERTY(QString emptyKind READ emptyKind CONSTANT)
    Q_PROPERTY(QString kindOnly READ kindOnly CONSTANT)
    Q_PROPERTY(QString noAccounts READ noAccounts CONSTANT)
    Q_PROPERTY(QString commands READ commands CONSTANT)
    Q_PROPERTY(QString shortcuts READ shortcuts CONSTANT)
    Q_PROPERTY(QString settings READ settings CONSTANT)
    Q_PROPERTY(QString feedSection READ feedSection CONSTANT)
    Q_PROPERTY(QString feedHint READ feedHint CONSTANT)
    Q_PROPERTY(QString indented READ indented CONSTANT)
    Q_PROPERTY(QString generalSection READ generalSection CONSTANT)
    Q_PROPERTY(QString generalHint READ generalHint CONSTANT)
    Q_PROPERTY(QString theme READ theme CONSTANT)
    Q_PROPERTY(QString themeSystem READ themeSystem CONSTANT)
    Q_PROPERTY(QString themeLight READ themeLight CONSTANT)
    Q_PROPERTY(QString themeDark READ themeDark CONSTANT)
    Q_PROPERTY(QString wheel READ wheel CONSTANT)
    Q_PROPERTY(QString wheelHint READ wheelHint CONSTANT)
    Q_PROPERTY(QString closeToTray READ closeToTray CONSTANT)
    Q_PROPERTY(QString closeToTrayHint READ closeToTrayHint CONSTANT)
    Q_PROPERTY(QString autostart READ autostart CONSTANT)
    Q_PROPERTY(QString flaggedFirst READ flaggedFirst CONSTANT)
    Q_PROPERTY(QString feedKinds READ feedKinds CONSTANT)
    Q_PROPERTY(QString feedKindsHint READ feedKindsHint CONSTANT)
    Q_PROPERTY(QString shadows READ shadows CONSTANT)
    Q_PROPERTY(QString shadowsMissing READ shadowsMissing CONSTANT)
    Q_PROPERTY(QString notifications READ notifications CONSTANT)
    Q_PROPERTY(QString notificationsHint READ notificationsHint CONSTANT)
    Q_PROPERTY(QString downloadFolder READ downloadFolder CONSTANT)
    Q_PROPERTY(QString downloadFolderHint READ downloadFolderHint CONSTANT)
    Q_PROPERTY(QString downloadAll READ downloadAll CONSTANT)
    Q_PROPERTY(QString noTray READ noTray CONSTANT)
    Q_PROPERTY(QString open READ open CONSTANT)
    Q_PROPERTY(QString quit READ quit CONSTANT)
    Q_PROPERTY(QString accountsSection READ accountsSection CONSTANT)
    Q_PROPERTY(QString accountsHint READ accountsHint CONSTANT)
    Q_PROPERTY(QString commandsSection READ commandsSection CONSTANT)
    Q_PROPERTY(QString commandsHint READ commandsHint CONSTANT)
    Q_PROPERTY(QString test READ test CONSTANT)
    Q_PROPERTY(QString edit READ edit CONSTANT)
    Q_PROPERTY(QString save READ save CONSTANT)
    Q_PROPERTY(QString cancel READ cancel CONSTANT)
    Q_PROPERTY(QString addAccount READ addAccount CONSTANT)
    Q_PROPERTY(QString add READ add CONSTANT)
    Q_PROPERTY(QString accountName READ accountName CONSTANT)
    Q_PROPERTY(QString accountNameHint READ accountNameHint CONSTANT)
    Q_PROPERTY(QString imapServer READ imapServer CONSTANT)
    Q_PROPERTY(QString smtpServer READ smtpServer CONSTANT)
    Q_PROPERTY(QString smtpHint READ smtpHint CONSTANT)
    Q_PROPERTY(QString login READ login CONSTANT)
    Q_PROPERTY(QString password READ password CONSTANT)
    Q_PROPERTY(QString passwordHint READ passwordHint CONSTANT)
    Q_PROPERTY(QString passwordMissing READ passwordMissing CONSTANT)
    Q_PROPERTY(QString noSmtp READ noSmtp CONSTANT)
    Q_PROPERTY(QString defaultPort READ defaultPort CONSTANT)
    Q_PROPERTY(QString copyToSentHint READ copyToSentHint CONSTANT)
    Q_PROPERTY(QString on READ on CONSTANT)
    Q_PROPERTY(QString off READ off CONSTANT)
    Q_PROPERTY(QString passwordUnknown READ passwordUnknown CONSTANT)
    Q_PROPERTY(QString sendingVia READ sendingVia CONSTANT)
    Q_PROPERTY(QString copyToSent READ copyToSent CONSTANT)
    Q_PROPERTY(QString archiveByYear READ archiveByYear CONSTANT)
    Q_PROPERTY(QString archiveByYearHint READ archiveByYearHint CONSTANT)
    Q_PROPERTY(QString color READ color CONSTANT)
    Q_PROPERTY(QString signature READ signature CONSTANT)
    Q_PROPERTY(QString signatureHint READ signatureHint CONSTANT)
    Q_PROPERTY(QString context READ context CONSTANT)
    Q_PROPERTY(QString contextHint READ contextHint CONSTANT)
    Q_PROPERTY(QString contextNone READ contextNone CONSTANT)
    Q_PROPERTY(QString contextChoose READ contextChoose CONSTANT)
    Q_PROPERTY(QString contextLimit READ contextLimit CONSTANT)
    Q_PROPERTY(QString contextLimitHint READ contextLimitHint CONSTANT)
    Q_PROPERTY(QString aiSection READ aiSection CONSTANT)
    Q_PROPERTY(QString aiHint READ aiHint CONSTANT)
    Q_PROPERTY(QString aiModel READ aiModel CONSTANT)
    Q_PROPERTY(QString aiPrivacyTitle READ aiPrivacyTitle CONSTANT)
    Q_PROPERTY(QString aiPrivacy READ aiPrivacy CONSTANT)
    Q_PROPERTY(QString aiProgram READ aiProgram CONSTANT)
    Q_PROPERTY(QString aiProgramHint READ aiProgramHint CONSTANT)
    Q_PROPERTY(QString aiProgramPlaceholder READ aiProgramPlaceholder CONSTANT)
    Q_PROPERTY(QString aiNotFound READ aiNotFound CONSTANT)
    Q_PROPERTY(QString aiNoModel READ aiNoModel CONSTANT)
    Q_PROPERTY(QString aiNotRunning READ aiNotRunning CONSTANT)
    Q_PROPERTY(QString aiAboutClaude READ aiAboutClaude CONSTANT)
    Q_PROPERTY(QString aiAboutOllama READ aiAboutOllama CONSTANT)
    Q_PROPERTY(QString removeAccount READ removeAccount CONSTANT)
    Q_PROPERTY(QString removeHint READ removeHint CONSTANT)
    Q_PROPERTY(QString purge READ purge CONSTANT)
    Q_PROPERTY(QString remove READ remove CONSTANT)
    Q_PROPERTY(QString defaultFolders READ defaultFolders CONSTANT)
    Q_PROPERTY(QString defaultFoldersHint READ defaultFoldersHint CONSTANT)
    Q_PROPERTY(QString kindsMissing READ kindsMissing CONSTANT)
    Q_PROPERTY(QString folderSync READ folderSync CONSTANT)
    Q_PROPERTY(QString folderSyncTip READ folderSyncTip CONSTANT)
    Q_PROPERTY(QString defaultFolder READ defaultFolder CONSTANT)
    Q_PROPERTY(QString defaultFolderTip READ defaultFolderTip CONSTANT)
    Q_PROPERTY(QString backTip READ backTip CONSTANT)
    Q_PROPERTY(QString searchTip READ searchTip CONSTANT)
    Q_PROPERTY(QString composeTip READ composeTip CONSTANT)
    Q_PROPERTY(QString settingsTip READ settingsTip CONSTANT)
    Q_PROPERTY(QString syncTip READ syncTip CONSTANT)
    Q_PROPERTY(QString starTip READ starTip CONSTANT)
    Q_PROPERTY(QString unstarTip READ unstarTip CONSTANT)
    Q_PROPERTY(QString unreadTip READ unreadTip CONSTANT)
    Q_PROPERTY(QString readTip READ readTip CONSTANT)
    Q_PROPERTY(QString archiveTip READ archiveTip CONSTANT)
    Q_PROPERTY(QString trashTip READ trashTip CONSTANT)
    Q_PROPERTY(QString inboxTip READ inboxTip CONSTANT)
    Q_PROPERTY(QString deleteTip READ deleteTip CONSTANT)
    Q_PROPERTY(QString quote READ quote CONSTANT)
    Q_PROPERTY(QString to READ to CONSTANT)
    Q_PROPERTY(QString replyAll READ replyAll CONSTANT)
    Q_PROPERTY(QString replyMode READ replyMode CONSTANT)
    Q_PROPERTY(QString forwardMode READ forwardMode CONSTANT)
    Q_PROPERTY(QString modeTip READ modeTip CONSTANT)
    Q_PROPERTY(QString menuCut READ menuCut CONSTANT)
    Q_PROPERTY(QString menuCopy READ menuCopy CONSTANT)
    Q_PROPERTY(QString menuPaste READ menuPaste CONSTANT)
    Q_PROPERTY(QString menuSelectAll READ menuSelectAll CONSTANT)
    Q_PROPERTY(QString ctrlKey READ ctrlKey CONSTANT)
    Q_PROPERTY(QString copied READ copied CONSTANT)
    Q_PROPERTY(QString attachTip READ attachTip CONSTANT)
    Q_PROPERTY(QString quoteCommand READ quoteCommand CONSTANT)
    Q_PROPERTY(QString quoteCommandHint READ quoteCommandHint CONSTANT)
    Q_PROPERTY(QString signatureCommand READ signatureCommand CONSTANT)
    Q_PROPERTY(QString signatureCommandHint READ signatureCommandHint CONSTANT)
    Q_PROPERTY(QString noSignature READ noSignature CONSTANT)
    Q_PROPERTY(QString attachCommand READ attachCommand CONSTANT)
    Q_PROPERTY(QString dateCommand READ dateCommand CONSTANT)
    Q_PROPERTY(QString dateCommandHint READ dateCommandHint CONSTANT)
    Q_PROPERTY(QString aiCommand READ aiCommand CONSTANT)
    Q_PROPERTY(QString aiCommandHint READ aiCommandHint CONSTANT)
    Q_PROPERTY(QString aiCommandOff READ aiCommandOff CONSTANT)
    Q_PROPERTY(QString aiCommandReplyOnly READ aiCommandReplyOnly CONSTANT)
    Q_PROPERTY(QString aiThinking READ aiThinking CONSTANT)
    Q_PROPERTY(QString aiStop READ aiStop CONSTANT)
    Q_PROPERTY(QString aiAsks READ aiAsks CONSTANT)
    Q_PROPERTY(QString aiAnswerPlaceholder READ aiAnswerPlaceholder CONSTANT)
    Q_PROPERTY(QString aiSkip READ aiSkip CONSTANT)
    Q_PROPERTY(QString gapLeft READ gapLeft CONSTANT)
    Q_PROPERTY(QString aiReportHide READ aiReportHide CONSTANT)
    Q_PROPERTY(QString aiReportTip READ aiReportTip CONSTANT)
    Q_PROPERTY(QString foldedLines READ foldedLines CONSTANT)
    Q_PROPERTY(QString foldTip READ foldTip CONSTANT)
    Q_PROPERTY(QString syncFolder READ syncFolder CONSTANT)
    Q_PROPERTY(QString replyPlaceholder READ replyPlaceholder CONSTANT)
    Q_PROPERTY(QString sendTip READ sendTip CONSTANT)
    Q_PROPERTY(QString saveDraftTip READ saveDraftTip CONSTANT)
    Q_PROPERTY(QString draftSaved READ draftSaved CONSTANT)
    Q_PROPERTY(QString discardTip READ discardTip CONSTANT)
    Q_PROPERTY(QString discarding READ discarding CONSTANT)
    Q_PROPERTY(QString deleting READ deleting CONSTANT)
    Q_PROPERTY(QString sending READ sending CONSTANT)
    Q_PROPERTY(QString undo READ undo CONSTANT)
    Q_PROPERTY(QString undoKey READ undoKey CONSTANT)
    Q_PROPERTY(QString upToDate READ upToDate CONSTANT)
    Q_PROPERTY(QString archived READ archived CONSTANT)
    Q_PROPERTY(QString trashed READ trashed CONSTANT)
    Q_PROPERTY(QString movedToInbox READ movedToInbox CONSTANT)
    Q_PROPERTY(QString current READ current CONSTANT)
    Q_PROPERTY(QString lastSyncTip READ lastSyncTip CONSTANT)
    Q_PROPERTY(QString closingAfterSend READ closingAfterSend CONSTANT)
    Q_PROPERTY(QString noText READ noText CONSTANT)
    Q_PROPERTY(QString newMail READ newMail CONSTANT)
    Q_PROPERTY(QString ccKind READ ccKind CONSTANT)
    Q_PROPERTY(QString bccKind READ bccKind CONSTANT)
    Q_PROPERTY(QString senderPlaceholder READ senderPlaceholder CONSTANT)
    Q_PROPERTY(QString subject READ subject CONSTANT)
    Q_PROPERTY(QString mailPlaceholder READ mailPlaceholder CONSTANT)
    Q_PROPERTY(QString addressPlaceholder READ addressPlaceholder CONSTANT)
    Q_PROPERTY(QString searchMail READ searchMail CONSTANT)
    Q_PROPERTY(QString searchContacts READ searchContacts CONSTANT)
    Q_PROPERTY(QString noContacts READ noContacts CONSTANT)
    Q_PROPERTY(QString noMatch READ noMatch CONSTANT)
    Q_PROPERTY(QString pickHint READ pickHint CONSTANT)

public:
    using QObject::QObject;

    QString wordmark() const { return QString::fromUtf8(texts().wordmark); }
    QString license() const { return QString::fromUtf8(texts().license); }
    QString licenseLink() const { return QString::fromUtf8(texts().licenseLink); }
    QString version() const { return QString::fromUtf8(texts().version); }
    QString infoSection() const { return QString::fromUtf8(texts().infoSection); }
    QString infoHint() const { return QString::fromUtf8(texts().infoHint); }
    QString sourceCode() const { return QString::fromUtf8(texts().sourceCode); }
    QString projectLink() const { return QString::fromUtf8(texts().projectLink); }
    QString issues() const { return QString::fromUtf8(texts().issues); }
    QString issuesLink() const { return QString::fromUtf8(texts().issuesLink); }
    QString licenseTitle() const { return QString::fromUtf8(texts().licenseTitle); }
    QString thirdParty() const { return QString::fromUtf8(texts().thirdParty); }
    QString thirdPartyHint() const { return QString::fromUtf8(texts().thirdPartyHint); }
    QString liabilityTitle() const { return QString::fromUtf8(texts().liabilityTitle); }
    QString liability() const { return QString::fromUtf8(texts().liability); }
    QString unread() const { return QString::fromUtf8(texts().unread); }
    QString hits() const { return QString::fromUtf8(texts().hits); }
    QString empty() const { return QString::fromUtf8(texts().empty); }
    QString noResults() const { return QString::fromUtf8(texts().noResults); }
    QString emptyKind() const { return QString::fromUtf8(texts().emptyKind); }
    QString kindOnly() const { return QString::fromUtf8(texts().kindOnly); }
    QString noAccounts() const { return QString::fromUtf8(texts().noAccounts); }
    QString commands() const { return QString::fromUtf8(texts().commands); }
    QString shortcuts() const { return QString::fromUtf8(texts().shortcuts); }
    QString settings() const { return QString::fromUtf8(texts().settings); }
    QString feedSection() const { return QString::fromUtf8(texts().feedSection); }
    QString feedHint() const { return QString::fromUtf8(texts().feedHint); }
    QString indented() const { return QString::fromUtf8(texts().indented); }
    QString generalSection() const { return QString::fromUtf8(texts().generalSection); }
    QString generalHint() const { return QString::fromUtf8(texts().generalHint); }
    QString theme() const { return QString::fromUtf8(texts().theme); }
    QString themeSystem() const { return QString::fromUtf8(texts().themeSystem); }
    QString themeLight() const { return QString::fromUtf8(texts().themeLight); }
    QString themeDark() const { return QString::fromUtf8(texts().themeDark); }
    QString wheel() const { return QString::fromUtf8(texts().wheel); }
    QString wheelHint() const { return QString::fromUtf8(texts().wheelHint); }
    QString closeToTray() const { return QString::fromUtf8(texts().closeToTray); }
    QString closeToTrayHint() const { return QString::fromUtf8(texts().closeToTrayHint); }
    QString autostart() const { return QString::fromUtf8(texts().autostart); }
    QString flaggedFirst() const { return QString::fromUtf8(texts().flaggedFirst); }
    QString feedKinds() const { return QString::fromUtf8(texts().feedKinds); }
    QString feedKindsHint() const { return QString::fromUtf8(texts().feedKindsHint); }
    QString shadows() const { return QString::fromUtf8(texts().shadows); }
    QString shadowsMissing() const { return QString::fromUtf8(texts().shadowsMissing); }
    QString notifications() const { return QString::fromUtf8(texts().notifications); }
    QString notificationsHint() const { return QString::fromUtf8(texts().notificationsHint); }
    QString downloadFolder() const { return QString::fromUtf8(texts().downloadFolder); }
    QString downloadFolderHint() const { return QString::fromUtf8(texts().downloadFolderHint); }
    QString downloadAll() const { return QString::fromUtf8(texts().downloadAll); }
    QString noTray() const { return QString::fromUtf8(texts().noTray); }
    QString open() const { return QString::fromUtf8(texts().open); }
    QString quit() const { return QString::fromUtf8(texts().quit); }
    QString accountsSection() const { return QString::fromUtf8(texts().accountsSection); }
    QString accountsHint() const { return QString::fromUtf8(texts().accountsHint); }
    QString commandsSection() const { return QString::fromUtf8(texts().commandsSection); }
    QString commandsHint() const { return QString::fromUtf8(texts().commandsHint); }
    QString test() const { return QString::fromUtf8(texts().test); }
    QString edit() const { return QString::fromUtf8(texts().edit); }
    QString save() const { return QString::fromUtf8(texts().save); }
    QString cancel() const { return QString::fromUtf8(texts().cancel); }
    QString addAccount() const { return QString::fromUtf8(texts().addAccount); }
    QString add() const { return QString::fromUtf8(texts().add); }
    QString accountName() const { return QString::fromUtf8(texts().accountName); }
    QString accountNameHint() const { return QString::fromUtf8(texts().accountNameHint); }
    QString imapServer() const { return QString::fromUtf8(texts().imapServer); }
    QString smtpServer() const { return QString::fromUtf8(texts().smtpServer); }
    QString smtpHint() const { return QString::fromUtf8(texts().smtpHint); }
    QString login() const { return QString::fromUtf8(texts().login); }
    QString password() const { return QString::fromUtf8(texts().password); }
    QString passwordHint() const { return QString::fromUtf8(texts().passwordHint); }
    QString passwordMissing() const { return QString::fromUtf8(texts().passwordMissing); }
    QString noSmtp() const { return QString::fromUtf8(texts().noSmtp); }
    QString defaultPort() const { return QString::fromUtf8(texts().defaultPort); }
    QString copyToSentHint() const { return QString::fromUtf8(texts().copyToSentHint); }
    QString on() const { return QString::fromUtf8(texts().on); }
    QString off() const { return QString::fromUtf8(texts().off); }
    QString passwordUnknown() const { return QString::fromUtf8(texts().passwordUnknown); }
    QString sendingVia() const { return QString::fromUtf8(texts().sendingVia); }
    QString copyToSent() const { return QString::fromUtf8(texts().copyToSent); }
    QString archiveByYear() const { return QString::fromUtf8(texts().archiveByYear); }
    QString archiveByYearHint() const { return QString::fromUtf8(texts().archiveByYearHint); }
    QString color() const { return QString::fromUtf8(texts().color); }
    QString signature() const { return QString::fromUtf8(texts().signature); }
    QString signatureHint() const { return QString::fromUtf8(texts().signatureHint); }
    QString context() const { return QString::fromUtf8(texts().context); }
    QString contextHint() const { return QString::fromUtf8(texts().contextHint); }
    QString contextNone() const { return QString::fromUtf8(texts().contextNone); }
    QString contextChoose() const { return QString::fromUtf8(texts().contextChoose); }
    QString contextLimit() const { return QString::fromUtf8(texts().contextLimit); }
    QString contextLimitHint() const { return QString::fromUtf8(texts().contextLimitHint); }
    QString aiSection() const { return QString::fromUtf8(texts().aiSection); }
    QString aiHint() const { return QString::fromUtf8(texts().aiHint); }
    QString aiModel() const { return QString::fromUtf8(texts().aiModel); }
    QString aiPrivacyTitle() const { return QString::fromUtf8(texts().aiPrivacyTitle); }
    QString aiPrivacy() const { return QString::fromUtf8(texts().aiPrivacy); }
    QString aiProgram() const { return QString::fromUtf8(texts().aiProgram); }
    QString aiProgramHint() const { return QString::fromUtf8(texts().aiProgramHint); }
    QString aiProgramPlaceholder() const { return QString::fromUtf8(texts().aiProgramPlaceholder); }
    QString aiNotFound() const { return QString::fromUtf8(texts().aiNotFound); }
    QString aiNoModel() const { return QString::fromUtf8(texts().aiNoModel); }
    QString aiNotRunning() const { return QString::fromUtf8(texts().aiNotRunning); }
    QString aiAboutClaude() const { return QString::fromUtf8(texts().aiAboutClaude); }
    QString aiAboutOllama() const { return QString::fromUtf8(texts().aiAboutOllama); }
    QString removeAccount() const { return QString::fromUtf8(texts().removeAccount); }
    QString removeHint() const { return QString::fromUtf8(texts().removeHint); }
    QString purge() const { return QString::fromUtf8(texts().purge); }
    QString remove() const { return QString::fromUtf8(texts().remove); }
    QString defaultFolders() const { return QString::fromUtf8(texts().defaultFolders); }
    QString defaultFoldersHint() const { return QString::fromUtf8(texts().defaultFoldersHint); }
    QString kindsMissing() const { return QString::fromUtf8(texts().kindsMissing); }
    QString folderSync() const { return QString::fromUtf8(texts().folderSync); }
    QString folderSyncTip() const { return QString::fromUtf8(texts().folderSyncTip); }
    QString defaultFolder() const { return QString::fromUtf8(texts().defaultFolder); }
    QString defaultFolderTip() const { return QString::fromUtf8(texts().defaultFolderTip); }
    QString backTip() const { return QString::fromUtf8(texts().backTip); }
    QString searchTip() const { return QString::fromUtf8(texts().searchTip); }
    QString composeTip() const { return QString::fromUtf8(texts().composeTip); }
    QString settingsTip() const { return QString::fromUtf8(texts().settingsTip); }
    QString syncTip() const { return QString::fromUtf8(texts().syncTip); }
    QString starTip() const { return QString::fromUtf8(texts().starTip); }
    QString unstarTip() const { return QString::fromUtf8(texts().unstarTip); }
    QString unreadTip() const { return QString::fromUtf8(texts().unreadTip); }
    QString readTip() const { return QString::fromUtf8(texts().readTip); }
    QString archiveTip() const { return QString::fromUtf8(texts().archiveTip); }
    QString trashTip() const { return QString::fromUtf8(texts().trashTip); }
    QString inboxTip() const { return QString::fromUtf8(texts().inboxTip); }
    QString deleteTip() const { return QString::fromUtf8(texts().deleteTip); }
    QString quote() const { return QString::fromUtf8(texts().quote); }
    QString to() const { return QString::fromUtf8(texts().to); }
    QString replyAll() const { return QString::fromUtf8(texts().replyAll); }
    QString replyMode() const { return QString::fromUtf8(texts().replyMode); }
    QString forwardMode() const { return QString::fromUtf8(texts().forwardMode); }
    QString modeTip() const { return QString::fromUtf8(texts().modeTip); }
    QString menuCut() const { return QString::fromUtf8(texts().menuCut); }
    QString menuCopy() const { return QString::fromUtf8(texts().menuCopy); }
    QString menuPaste() const { return QString::fromUtf8(texts().menuPaste); }
    QString menuSelectAll() const { return QString::fromUtf8(texts().menuSelectAll); }
    QString ctrlKey() const { return QString::fromUtf8(texts().ctrlKey); }
    QString copied() const { return QString::fromUtf8(texts().copied); }
    QString attachTip() const { return QString::fromUtf8(texts().attachTip); }
    QString quoteCommand() const { return QString::fromUtf8(texts().quoteCommand); }
    QString quoteCommandHint() const { return QString::fromUtf8(texts().quoteCommandHint); }
    QString signatureCommand() const { return QString::fromUtf8(texts().signatureCommand); }
    QString signatureCommandHint() const { return QString::fromUtf8(texts().signatureCommandHint); }
    QString noSignature() const { return QString::fromUtf8(texts().noSignature); }
    QString attachCommand() const { return QString::fromUtf8(texts().attachCommand); }
    QString dateCommand() const { return QString::fromUtf8(texts().dateCommand); }
    QString dateCommandHint() const { return QString::fromUtf8(texts().dateCommandHint); }
    QString aiCommand() const { return QString::fromUtf8(texts().aiCommand); }
    QString aiCommandHint() const { return QString::fromUtf8(texts().aiCommandHint); }
    QString aiCommandOff() const { return QString::fromUtf8(texts().aiCommandOff); }
    QString aiCommandReplyOnly() const { return QString::fromUtf8(texts().aiCommandReplyOnly); }
    QString aiThinking() const { return QString::fromUtf8(texts().aiThinking); }
    QString aiStop() const { return QString::fromUtf8(texts().aiStop); }
    QString aiAsks() const { return QString::fromUtf8(texts().aiAsks); }
    QString aiAnswerPlaceholder() const { return QString::fromUtf8(texts().aiAnswerPlaceholder); }
    QString aiSkip() const { return QString::fromUtf8(texts().aiSkip); }
    QString gapLeft() const { return QString::fromUtf8(texts().gapLeft); }
    QString aiReportHide() const { return QString::fromUtf8(texts().aiReportHide); }
    QString aiReportTip() const { return QString::fromUtf8(texts().aiReportTip); }
    Q_INVOKABLE QVariantList dates(const QDateTime& when) const;
    QString foldedLines() const { return QString::fromUtf8(texts().foldedLines); }
    QString foldTip() const { return QString::fromUtf8(texts().foldTip); }
    QString syncFolder() const { return QString::fromUtf8(texts().syncFolder); }
    QString replyPlaceholder() const { return QString::fromUtf8(texts().replyPlaceholder); }
    QString sendTip() const { return QString::fromUtf8(texts().sendTip); }
    QString saveDraftTip() const { return QString::fromUtf8(texts().saveDraftTip); }
    QString draftSaved() const { return QString::fromUtf8(texts().draftSaved); }
    QString discardTip() const { return QString::fromUtf8(texts().discardTip); }
    QString discarding() const { return QString::fromUtf8(texts().discarding); }
    QString deleting() const { return QString::fromUtf8(texts().deleting); }
    QString sending() const { return QString::fromUtf8(texts().sending); }
    QString undo() const { return QString::fromUtf8(texts().undo); }
    QString undoKey() const { return QString::fromUtf8(texts().undoKey); }
    QString upToDate() const { return QString::fromUtf8(texts().upToDate); }
    QString archived() const { return QString::fromUtf8(texts().archived); }
    QString trashed() const { return QString::fromUtf8(texts().trashed); }
    QString movedToInbox() const { return QString::fromUtf8(texts().movedToInbox); }
    QString current() const { return QString::fromUtf8(texts().current); }
    QString lastSyncTip() const { return QString::fromUtf8(texts().lastSyncTip); }
    QString closingAfterSend() const { return QString::fromUtf8(texts().closingAfterSend); }
    QString noText() const { return QString::fromUtf8(texts().noText); }
    QString newMail() const { return QString::fromUtf8(texts().newMail); }
    QString ccKind() const { return QString::fromUtf8(texts().ccKind); }
    QString bccKind() const { return QString::fromUtf8(texts().bccKind); }
    QString senderPlaceholder() const { return QString::fromUtf8(texts().senderPlaceholder); }
    QString subject() const { return QString::fromUtf8(texts().subject); }
    QString mailPlaceholder() const { return QString::fromUtf8(texts().mailPlaceholder); }
    QString addressPlaceholder() const { return QString::fromUtf8(texts().addressPlaceholder); }
    QString searchMail() const { return QString::fromUtf8(texts().searchMail); }
    QString searchContacts() const { return QString::fromUtf8(texts().searchContacts); }
    QString noContacts() const { return QString::fromUtf8(texts().noContacts); }
    QString noMatch() const { return QString::fromUtf8(texts().noMatch); }
    QString pickHint() const { return QString::fromUtf8(texts().pickHint); }
};
