//! Landing page component
//! Matches NexusForge styling

use leptos::*;
use leptos_router::*;

/// Landing page component
#[component]
pub fn Landing() -> impl IntoView {
    let navigate = use_navigate();
    let nav1 = navigate.clone();
    let nav2 = navigate.clone();
    let nav3 = navigate.clone();
    let nav4 = navigate.clone();

    view! {
        <div class="landing-container">
            // Header / Nav
            <header class="landing-header">
                <nav class="landing-nav">
                    <div class="logo-container">
                        <svg class="landing-logo" viewBox="0 0 100 100" width="40" height="40">
                            <defs>
                                <linearGradient id="grad" x1="0%" y1="0%" x2="100%" y2="100%">
                                    <stop offset="0%" style="stop-color:#14b8a6"/>
                                    <stop offset="100%" style="stop-color:#0d9488"/>
                                </linearGradient>
                            </defs>
                            <rect width="100" height="100" rx="20" fill="url(#grad)"/>
                            <path d="M30 35 L50 25 L70 35 L70 65 L50 75 L30 65 Z" fill="none" stroke="white" stroke-width="4" stroke-linejoin="round"/>
                            <path d="M30 45 L50 35 L70 45" fill="none" stroke="white" stroke-width="3"/>
                            <path d="M30 55 L50 45 L70 55" fill="none" stroke="white" stroke-width="3"/>
                            <circle cx="50" cy="50" r="6" fill="white"/>
                        </svg>
                        <span class="logo-text">"Aegis DB"</span>
                    </div>
                    <div class="nav-buttons">
                        <button class="btn-secondary" on:click=move |_| nav1("/login", Default::default())>"Sign In"</button>
                        <button class="btn-primary" on:click=move |_| nav2("/login", Default::default())>"Get Started"</button>
                    </div>
                </nav>

                // Hero Section
                <div class="hero-content">
                    <div class="hero-badge">"Multi-Paradigm Distributed Database"</div>
                    <h1>"The Database Platform Built for Modern Applications"</h1>
                    <p class="hero-subtitle">
                        "Key-Value, Document, Graph, and Time-Series in one unified platform. "
                        "Powered by "<strong>"Raft consensus"</strong>" for bulletproof reliability."
                    </p>
                    <div class="hero-cta">
                        <button class="btn-primary btn-large" on:click=move |_| nav3("/login", Default::default())>
                            "Start Free "
                            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <path d="M5 12h14M12 5l7 7-7 7"/>
                            </svg>
                        </button>
                        <span class="cta-note">"No credit card required"</span>
                    </div>
                </div>
            </header>

            // Features Section
            <section class="features-section">
                <h2>"One Database, Infinite Possibilities"</h2>
                <div class="features-grid">
                    <FeatureCard
                        icon="database"
                        title="Multi-Paradigm Storage"
                        description="Key-Value for speed, Documents for flexibility, Graphs for relationships, Time-Series for metrics. All in one place."
                    />
                    <FeatureCard
                        icon="network"
                        title="Raft Consensus"
                        description="Production-grade distributed consensus ensures your data is safe, replicated, and always consistent across nodes."
                    />
                    <FeatureCard
                        icon="zap"
                        title="Blazing Fast"
                        description="Written in Rust for zero-cost abstractions. Sub-millisecond reads with intelligent caching and optimized storage."
                    />
                    <FeatureCard
                        icon="shield"
                        title="Enterprise Security"
                        description="RBAC, encryption at rest and in transit, audit logging, and MFA authentication built in from day one."
                    />
                    <FeatureCard
                        icon="chart"
                        title="Real-Time Metrics"
                        description="Prometheus-compatible metrics, distributed tracing, and comprehensive health monitoring out of the box."
                    />
                    <FeatureCard
                        icon="layers"
                        title="Horizontal Scaling"
                        description="Automatic sharding, consistent hashing, and intelligent data placement for seamless cluster expansion."
                    />
                </div>
            </section>

            // How It Works Section
            <section class="how-it-works">
                <h2>"How Aegis DB Works"</h2>
                <div class="steps-container">
                    <div class="step">
                        <div class="step-number">"1"</div>
                        <h3>"Deploy Your Cluster"</h3>
                        <p>"Spin up nodes with Helm, Docker, or bare metal. Automatic leader election handles the rest."</p>
                    </div>
                    <div class="step">
                        <div class="step-number">"2"</div>
                        <h3>"Choose Your Paradigm"</h3>
                        <p>"Use the right data model for each use case. Mix and match within the same cluster."</p>
                    </div>
                    <div class="step">
                        <div class="step-number">"3"</div>
                        <h3>"Scale With Confidence"</h3>
                        <p>"Add nodes, enable sharding, and let Aegis handle replication and consistency automatically."</p>
                    </div>
                </div>
            </section>

            // Benefits Section
            <section class="benefits-section">
                <h2>"Why Choose Aegis DB?"</h2>
                <div class="benefits-list">
                    <BenefitItem text="Zero-downtime deployments with rolling updates"/>
                    <BenefitItem text="ACID transactions across data paradigms"/>
                    <BenefitItem text="CRDTs for eventual consistency workloads"/>
                    <BenefitItem text="Native Kubernetes integration with Helm charts"/>
                    <BenefitItem text="GraphQL and REST APIs included"/>
                    <BenefitItem text="Active-active geo-replication"/>
                </div>
            </section>

            // CTA Section
            <section class="cta-section">
                <svg class="cta-icon" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/>
                    <line x1="8" y1="21" x2="16" y2="21"/>
                    <line x1="12" y1="17" x2="12" y2="21"/>
                </svg>
                <h2>"Ready to Build with Aegis?"</h2>
                <p>"Join teams using Aegis DB for their most critical data workloads."</p>
                <button class="btn-primary btn-large" on:click=move |_| nav4("/login", Default::default())>
                    "Get Started Free "
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M5 12h14M12 5l7 7-7 7"/>
                    </svg>
                </button>
            </section>

            // Footer
            <footer class="landing-footer">
                <div class="footer-content">
                    <div class="footer-brand">
                        <svg width="28" height="28" viewBox="0 0 100 100">
                            <defs>
                                <linearGradient id="grad2" x1="0%" y1="0%" x2="100%" y2="100%">
                                    <stop offset="0%" style="stop-color:#14b8a6"/>
                                    <stop offset="100%" style="stop-color:#0d9488"/>
                                </linearGradient>
                            </defs>
                            <rect width="100" height="100" rx="20" fill="url(#grad2)"/>
                            <path d="M30 35 L50 25 L70 35 L70 65 L50 75 L30 65 Z" fill="none" stroke="white" stroke-width="4" stroke-linejoin="round"/>
                            <circle cx="50" cy="50" r="6" fill="white"/>
                        </svg>
                        <span>"Aegis DB"</span>
                    </div>
                    <div class="footer-links">
                        <a href="https://github.com/aegisdb" target="_blank">"GitHub"</a>
                        <a href="/docs" target="_blank">"Documentation"</a>
                        <a href="https://automatanexus.com" target="_blank">"AutomataNexus"</a>
                    </div>
                    <div class="footer-copyright">
                        <p>"Copyright 2025 AutomataNexus, LLC. All Rights Reserved."</p>
                    </div>
                </div>
            </footer>
        </div>
    }
}

/// Feature card component
#[component]
fn FeatureCard(
    icon: &'static str,
    title: &'static str,
    description: &'static str,
) -> impl IntoView {
    let icon_svg = match icon {
        "database" => view! {
            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <ellipse cx="12" cy="5" rx="9" ry="3"/>
                <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/>
                <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/>
            </svg>
        }.into_view(),
        "network" => view! {
            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="3"/>
                <circle cx="19" cy="5" r="2"/>
                <circle cx="5" cy="5" r="2"/>
                <circle cx="19" cy="19" r="2"/>
                <circle cx="5" cy="19" r="2"/>
                <line x1="12" y1="9" x2="12" y2="3"/>
                <line x1="14.5" y1="10.5" x2="17.5" y2="6.5"/>
                <line x1="9.5" y1="10.5" x2="6.5" y2="6.5"/>
                <line x1="14.5" y1="13.5" x2="17.5" y2="17.5"/>
                <line x1="9.5" y1="13.5" x2="6.5" y2="17.5"/>
            </svg>
        }.into_view(),
        "zap" => view! {
            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
            </svg>
        }.into_view(),
        "shield" => view! {
            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
            </svg>
        }.into_view(),
        "chart" => view! {
            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="20" x2="18" y2="10"/>
                <line x1="12" y1="20" x2="12" y2="4"/>
                <line x1="6" y1="20" x2="6" y2="14"/>
            </svg>
        }.into_view(),
        "layers" => view! {
            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polygon points="12 2 2 7 12 12 22 7 12 2"/>
                <polyline points="2 17 12 22 22 17"/>
                <polyline points="2 12 12 17 22 12"/>
            </svg>
        }.into_view(),
        _ => view! { <span></span> }.into_view(),
    };

    view! {
        <div class="feature-card">
            <div class="feature-icon">{icon_svg}</div>
            <h3>{title}</h3>
            <p>{description}</p>
        </div>
    }
}

/// Benefit item component
#[component]
fn BenefitItem(text: &'static str) -> impl IntoView {
    view! {
        <div class="benefit-item">
            <svg class="benefit-icon" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/>
                <polyline points="22 4 12 14.01 9 11.01"/>
            </svg>
            <span>{text}</span>
        </div>
    }
}
