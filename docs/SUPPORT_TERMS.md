---
layout: default
title: Support Terms
nav_order: 9
description: "Support terms and conditions for Aegis-DB subscriptions"
---

# Support Terms and Conditions
{: .no_toc }

**Effective Date:** January 2025 | **Version:** 1.0
{: .fs-6 .fw-300 }

---

## Table of Contents
{: .no_toc .text-delta }

1. TOC
{:toc}

---

These Support Terms and Conditions ("Support Terms") govern the provision of technical support services ("Support Services") by Automata Nexus ("Provider," "we," "us," or "our") to customers ("Customer," "you," or "your") who have purchased a valid support subscription for Aegis-DB.

---

## 1. Definitions

**1.1 "Support Request"** means a formal communication from Customer to Provider through an authorized support channel, describing an issue, question, or request for assistance related to the covered Aegis-DB software.

**1.2 "Severity Level"** means the classification assigned to a Support Request based on the impact and urgency of the reported issue, as defined in Section 2.

**1.3 "Response Time"** means the elapsed time between Provider's receipt of a Support Request through an authorized channel and Provider's initial substantive response acknowledging the issue and providing preliminary guidance or requesting additional information.

**1.4 "Resolution Time"** means the target elapsed time for Provider to deliver a fix, workaround, or solution that restores the affected functionality to operational status. Resolution Times are targets, not guarantees, and may vary based on issue complexity.

**1.5 "Business Hours"** means Monday through Friday, 9:00 AM to 6:00 PM in the Customer's designated time zone, excluding public holidays observed by Provider. Enterprise tier customers receive 24/7 support for Severity 1 issues.

**1.6 "Workaround"** means a temporary solution or set of procedures that allows Customer to continue operations while a permanent fix is being developed.

**1.7 "Fix"** means a permanent correction to the software that resolves the reported issue.

**1.8 "Authorized Contact"** means an individual designated by Customer who is authorized to submit Support Requests and receive support communications on Customer's behalf.

**1.9 "Supported Version"** means a version of Aegis-DB that is currently within its support lifecycle as published in Provider's version support policy.

---

## 2. Severity Levels

All Support Requests are classified according to the following severity levels. Provider reserves the right to reclassify severity levels based on technical assessment.

### 2.1 Critical (Severity 1)

**Definition:** A production system is completely down, or a critical business function is unavailable with no workaround. This includes:

- Complete system outage preventing all database operations
- Data corruption or imminent risk of data loss
- Security breach or active exploitation of a vulnerability
- Cluster failure affecting all nodes
- Complete inability to start the Aegis-DB server

**Customer Obligations:** Customer must have technical personnel available 24/7 to work with Provider until the issue is resolved or downgraded.

### 2.2 High (Severity 2)

**Definition:** A major feature or function is severely impaired, but a workaround exists or the system remains partially operational. This includes:

- Significant performance degradation (>50% reduction in throughput)
- Replication failures affecting data consistency
- Authentication or authorization system failures
- Single node failure in a multi-node cluster
- Critical API endpoints returning errors intermittently

**Customer Obligations:** Customer must have technical personnel available during Business Hours to assist with troubleshooting.

### 2.3 Medium (Severity 3)

**Definition:** A feature or function is not operating as documented, but the impact on operations is limited. This includes:

- Non-critical features not functioning correctly
- Performance issues not affecting production SLAs
- Dashboard or monitoring display errors
- Query optimizer producing suboptimal plans
- Minor data import/export issues

### 2.4 Low (Severity 4)

**Definition:** General questions, documentation requests, feature requests, or minor issues with negligible operational impact. This includes:

- How-to questions and best practice guidance
- Documentation clarification requests
- Feature enhancement requests
- Cosmetic issues in user interfaces
- Minor bugs with simple workarounds

---

## 3. Response Time SLAs by Tier

Response Times are measured from the time a Support Request is received through an authorized channel during applicable support hours.

| Support Tier | Severity 1 | Severity 2 | Severity 3 | Severity 4 |
|--------------|------------|------------|------------|------------|
| **Professional** | 24 hours | 48 hours | 72 hours | 5 business days |
| **Business** | 4 hours | 8 hours | 24 hours | 48 hours |
| **Enterprise** | 1 hour | 4 hours | 8 hours | 24 hours |

### 3.1 Professional Tier

- Response Times apply during Business Hours only
- Support requests submitted outside Business Hours will be addressed on the next business day
- Monthly limit of 10 Support Requests (additional requests billed separately)

### 3.2 Business Tier

- Severity 1 and 2 Response Times apply 24/7
- Severity 3 and 4 Response Times apply during Business Hours
- Unlimited Support Requests
- Named support engineer assigned to account

### 3.3 Enterprise Tier

- All Response Times apply 24/7/365
- Unlimited Support Requests
- Dedicated support team with direct escalation paths
- Quarterly business reviews included
- Custom SLA terms available upon request

### 3.4 Target Resolution Times

The following are target resolution times. Actual resolution times depend on issue complexity and may require Customer cooperation.

| Severity Level | Target Resolution |
|----------------|-------------------|
| Severity 1 | 4-24 hours |
| Severity 2 | 1-3 business days |
| Severity 3 | 5-10 business days |
| Severity 4 | Best effort |

---

## 4. Support Scope

### 4.1 Covered Services

The following services are included in all support tiers:

**Installation and Deployment**
- Assistance with initial installation and configuration
- Guidance on deployment architecture and best practices
- Help with upgrade and migration procedures
- Troubleshooting installation failures

**Configuration and Optimization**
- Configuration parameter guidance
- Performance tuning recommendations
- Cluster configuration assistance
- Replication setup and configuration

**Bug Reports and Defect Resolution**
- Investigation of reported software defects
- Provision of hotfixes for critical bugs (Enterprise tier)
- Workaround guidance while fixes are developed
- Inclusion of fixes in future releases

**Performance Issues**
- Analysis of performance degradation reports
- Query optimization recommendations
- Resource utilization guidance
- Bottleneck identification assistance

**Security Vulnerabilities**
- Triage and assessment of reported vulnerabilities
- Security patch distribution
- Guidance on security hardening
- Incident response coordination (Enterprise tier)

**Operational Support**
- Backup and recovery guidance
- Monitoring and alerting configuration
- Log analysis assistance
- Disaster recovery planning guidance

### 4.2 Services Not Covered

The following services are outside the scope of standard support and require a separate consulting engagement:

**Custom Development**
- Custom feature development
- Custom plugin or extension development
- Integration code development
- Custom tool or utility development
- Schema design services

**Third-Party Integrations**
- Development of integrations with third-party systems
- Troubleshooting third-party software issues
- Compatibility testing with unsupported platforms
- Custom driver or connector development

**Training Services**
- On-site or virtual training sessions
- Custom training material development
- Certification programs
- Workshops and boot camps

*Note: Training services are available as a separate offering. Contact your account representative for details.*

**Infrastructure and Hardware**
- Operating system administration
- Network configuration and troubleshooting
- Hardware diagnostics and repair
- Cloud provider infrastructure issues
- Virtualization platform issues
- Container orchestration platform issues (except as related to Aegis-DB configuration)

**Unsupported Configurations**
- End-of-life software versions
- Unsupported operating systems or platforms
- Modified or forked source code
- Configurations not documented as supported

**Data Services**
- Data migration execution
- Data recovery from corrupted media
- Data cleansing or transformation
- Database administration tasks

---

## 5. Support Channels

### 5.1 Channel Availability by Tier

| Channel | Professional | Business | Enterprise |
|---------|--------------|----------|------------|
| Email Support | Yes | Yes | Yes |
| Support Portal | Yes | Yes | Yes |
| Slack/Teams | No | Yes | Yes |
| Phone Support | No | No | Yes |
| 24/7 On-Call | No | No | Yes (Sev 1) |
| Dedicated Slack Channel | No | No | Yes |
| Screen Sharing | Limited | Yes | Yes |

### 5.2 Email Support

- **Address:** Devops@automatanexus.com
- **Expected Format:** Include account ID, severity level, detailed issue description, reproduction steps, and relevant logs
- **Acknowledgment:** Automated acknowledgment within 15 minutes during Business Hours

### 5.3 Support Portal

- **URL:** https://support.automatanexus.com
- **Features:** Ticket submission, status tracking, knowledge base access, release notes
- **Account Setup:** Authorized Contacts will receive portal credentials upon subscription activation

### 5.4 Slack/Teams Integration (Business and Enterprise)

- Dedicated channel for real-time communication
- Direct access to support engineers
- Not intended for Severity 1 issues (use phone for Enterprise)

### 5.5 Phone Support (Enterprise Only)

- **Number:** Provided upon subscription activation
- **Availability:** 24/7 for Severity 1 issues; Business Hours for other severities
- **Callback:** If no immediate answer, callback within 15 minutes

### 5.6 On-Call Support (Enterprise Only)

- 24/7/365 availability for Severity 1 issues
- Direct pager access for critical production emergencies
- 15-minute response target for initial contact

---

## 6. Escalation Process

### 6.1 Standard Escalation Path

Support Requests follow this escalation path:

1. **Tier 1 - Support Engineer:** Initial triage, known issue identification, basic troubleshooting
2. **Tier 2 - Senior Support Engineer:** Complex troubleshooting, log analysis, workaround development
3. **Tier 3 - Engineering Team:** Code-level investigation, bug fixes, patch development
4. **Tier 4 - Engineering Leadership:** Critical issue coordination, resource allocation

### 6.2 Automatic Escalation

Support Requests are automatically escalated when:

- Response Time SLA is at risk (escalated to support management)
- Resolution Time target exceeded by 50% (escalated to Tier 2)
- Severity 1 issue unresolved after 4 hours (escalated to Engineering)
- Customer requests escalation

### 6.3 Customer-Initiated Escalation

Customers may request escalation at any time by:

- **Professional/Business:** Contacting support-escalation@automatanexus.com
- **Enterprise:** Contacting assigned Technical Account Manager or using dedicated escalation line

### 6.4 Executive Escalation (Enterprise Only)

For unresolved critical issues, Enterprise customers may escalate to:

- VP of Customer Success (after 24 hours unresolved)
- Chief Technology Officer (after 48 hours unresolved)

---

## 7. Maintenance Windows

### 7.1 Scheduled Maintenance

Provider may perform scheduled maintenance on support infrastructure during the following windows:

- **Primary Window:** Sundays, 2:00 AM - 6:00 AM UTC
- **Secondary Window:** First Saturday of each month, 2:00 AM - 8:00 AM UTC

Scheduled maintenance will be announced at least 72 hours in advance via:
- Support portal notification
- Email to Authorized Contacts
- Status page update (status.automatanexus.com)

### 7.2 Emergency Maintenance

Emergency maintenance may be performed without advance notice when required to:
- Address critical security vulnerabilities
- Prevent imminent service disruption
- Restore service after an outage

Customers will be notified as soon as practicable via status page and email.

### 7.3 Impact on SLAs

Time during scheduled maintenance windows is excluded from Response Time and Resolution Time calculations. Emergency maintenance time is included in SLA calculations.

---

## 8. Exclusions and Limitations

### 8.1 Support Exclusions

Provider is not obligated to provide support for issues arising from:

- Use of Aegis-DB in a manner not in accordance with documentation
- Modifications to Aegis-DB source code not authorized by Provider
- Third-party software, hardware, or services not supported by Provider
- Customer's failure to implement recommended fixes, patches, or upgrades
- Customer's failure to maintain supported versions
- Force majeure events
- Customer's failure to provide required access, information, or cooperation
- Issues previously resolved where Customer declined to implement the solution

### 8.2 Version Support Policy

- **Current Version (N):** Full support
- **Previous Version (N-1):** Full support for 12 months after N release
- **Older Versions (N-2 and earlier):** Limited support; upgrade required for full support

Customers running unsupported versions may receive limited assistance, and Response Time SLAs do not apply.

### 8.3 Limitation of Liability

Support Services are provided "as is" to the extent permitted by applicable law. Provider's liability for support-related issues is limited to the support fees paid by Customer in the 12 months preceding the claim.

Provider does not guarantee:
- That all issues will be resolved
- Specific resolution timeframes (targets only)
- Compatibility with future software or hardware
- Uninterrupted availability of support channels

### 8.4 Customer Responsibilities

Customer agrees to:

- Designate qualified Authorized Contacts
- Provide timely and accurate information about issues
- Implement recommended fixes and workarounds
- Maintain current contact information
- Ensure Authorized Contacts are available for Severity 1 and 2 issues
- Maintain backups per Provider's recommendations
- Apply security patches in a timely manner

---

## 9. Term and Termination

### 9.1 Term

Support subscriptions are sold in annual terms unless otherwise specified in the applicable order form. Subscriptions automatically renew for successive one-year terms unless either party provides written notice of non-renewal at least 30 days before the end of the current term.

### 9.2 Termination for Convenience

Either party may terminate the support subscription at the end of the current term by providing written notice as specified in Section 9.1.

### 9.3 Termination for Cause

Either party may terminate immediately upon written notice if:

- The other party materially breaches these Support Terms and fails to cure such breach within 30 days of written notice
- The other party becomes insolvent, files for bankruptcy, or ceases operations

### 9.4 Effect of Termination

Upon termination or expiration:

- Customer's access to support channels will be deactivated
- Open Support Requests will be closed without resolution (unless otherwise agreed)
- Customer retains access to public documentation and community resources
- No refunds are provided for unused portions of prepaid support terms, except as required by law

### 9.5 Survival

Sections relating to Definitions, Exclusions and Limitations, and any accrued obligations survive termination.

---

## 10. General Provisions

### 10.1 Modifications

Provider reserves the right to modify these Support Terms with 30 days' notice to Customers. Continued use of Support Services after modifications take effect constitutes acceptance of the modified terms.

### 10.2 Assignment

Customer may not assign its support subscription without Provider's prior written consent. Provider may assign its obligations to an affiliate or successor entity.

### 10.3 Entire Agreement

These Support Terms, together with the applicable order form and license agreement, constitute the entire agreement between the parties regarding Support Services.

### 10.4 Governing Law

These Support Terms are governed by the laws of the State of Delaware, USA, without regard to conflict of law principles.

### 10.5 Contact Information

For questions about these Support Terms:

- **Email:** legal@automatanexus.com
- **Address:** Automata Nexus, Inc., [Address on File]

---

## Appendix A: Severity Classification Examples

| Scenario | Severity | Rationale |
|----------|----------|-----------|
| All cluster nodes unresponsive | 1 | Complete production outage |
| Data corruption detected | 1 | Risk of data loss |
| Primary node failover not completing | 1 | Production at risk |
| Read replicas not syncing | 2 | Data consistency risk, but writes functional |
| Query performance degraded 60% | 2 | Significant impact, workaround possible |
| Dashboard charts not rendering | 3 | Monitoring affected, core DB functional |
| CLI command returns incorrect help text | 4 | Minor documentation issue |
| Feature request for new function | 4 | Enhancement, not defect |

---

## Appendix B: Information Required for Support Requests

To ensure efficient resolution, include the following in all Support Requests:

**Required:**
- Account ID and company name
- Aegis-DB version number
- Operating system and version
- Deployment type (single node, cluster, cloud)
- Severity level assessment
- Detailed issue description
- Steps to reproduce (if applicable)
- Error messages and log excerpts

**Recommended:**
- Recent configuration changes
- Workload characteristics
- Timeline of issue occurrence
- Impact assessment
- Workarounds already attempted

---

*© 2025 Automata Nexus, Inc. All rights reserved. Aegis-DB is a trademark of Automata Nexus, Inc.*
