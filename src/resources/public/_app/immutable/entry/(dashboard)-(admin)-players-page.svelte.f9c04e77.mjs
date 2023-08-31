import{S as re,i as ne,s as oe,y as G,z as I,A as Y,g as O,d as Q,B as j,I as se,a as B,k as v,q as R,c as N,l as T,m as w,r as V,h as m,n as L,b as S,G as i,v as ie,f as fe,N as ce,w as W,O as X,P as Z,u as C}from"../chunks/index.6f0c0f63.mjs";import{g as ue,a as x,p as _e}from"../chunks/api.ab561f20.mjs";import{D as he}from"../chunks/DashboardPage.c4ea9bf6.mjs";import{L as de}from"../chunks/Loader.f33e46d2.mjs";import{Q as me}from"../chunks/QueryPagination.d8a75d00.mjs";function ee(o,e,a){const t=o.slice();return t[11]=e[a],t}function te(o){let e,a;return e=new de({}),{c(){G(e.$$.fragment)},l(t){I(e.$$.fragment,t)},m(t,l){Y(e,t,l),a=!0},i(t){a||(O(e.$$.fragment,t),a=!0)},o(t){Q(e.$$.fragment,t),a=!1},d(t){j(e,t)}}}function ae(o){let e,a,t;return{c(){e=v("a"),a=R("View"),this.h()},l(l){e=T(l,"A",{class:!0,href:!0});var f=w(e);a=V(f,"View"),f.forEach(m),this.h()},h(){L(e,"class","button"),L(e,"href",t=`/players/${o[11].id}`)},m(l,f){S(l,e,f),i(e,a)},p(l,f){f&8&&t!==(t=`/players/${l[11].id}`)&&L(e,"href",t)},d(l){l&&m(e)}}}function le(o){let e,a,t=o[11].display_name+"",l,f,y,u=o[11].email+"",n,p,g,k=o[11].role+"",P,b,E,q=x(o[5],o[11]),H,c=q&&ae(o);return{c(){e=v("tr"),a=v("td"),l=R(t),f=B(),y=v("td"),n=R(u),p=B(),g=v("td"),P=R(k),b=B(),E=v("td"),c&&c.c(),H=B(),this.h()},l(d){e=T(d,"TR",{class:!0});var r=w(e);a=T(r,"TD",{});var A=w(a);l=V(A,t),A.forEach(m),f=N(r),y=T(r,"TD",{});var _=w(y);n=V(_,u),_.forEach(m),p=N(r),g=T(r,"TD",{});var s=w(g);P=V(s,k),s.forEach(m),b=N(r),E=T(r,"TD",{});var $=w(E);c&&c.l($),$.forEach(m),H=N(r),r.forEach(m),this.h()},h(){L(e,"class","table__entry")},m(d,r){S(d,e,r),i(e,a),i(a,l),i(e,f),i(e,y),i(y,n),i(e,p),i(e,g),i(g,P),i(e,b),i(e,E),c&&c.m(E,null),i(e,H)},p(d,r){r&8&&t!==(t=d[11].display_name+"")&&C(l,t),r&8&&u!==(u=d[11].email+"")&&C(n,u),r&8&&k!==(k=d[11].role+"")&&C(P,k),r&40&&(q=x(d[5],d[11])),q?c?c.p(d,r):(c=ae(d),c.c(),c.m(E,null)):c&&(c.d(1),c=null)},d(d){d&&m(e),c&&c.d()}}}function pe(o){let e,a,t,l,f,y,u,n,p,g,k,P,b,E,q,H,c,d,r=o[2]&&te(),A=o[3],_=[];for(let s=0;s<A.length;s+=1)_[s]=le(ee(o,A,s));return{c(){r&&r.c(),e=B(),a=v("table"),t=v("thead"),l=v("tr"),f=v("th"),y=R("Name"),u=B(),n=v("th"),p=R("Email"),g=B(),k=v("th"),P=R("Role"),b=B(),E=v("th"),q=R("View"),H=B(),c=v("tbody");for(let s=0;s<_.length;s+=1)_[s].c();this.h()},l(s){r&&r.l(s),e=N(s),a=T(s,"TABLE",{class:!0});var $=w(a);t=T($,"THEAD",{class:!0});var h=w(t);l=T(h,"TR",{});var D=w(l);f=T(D,"TH",{});var F=w(f);y=V(F,"Name"),F.forEach(m),u=N(D),n=T(D,"TH",{});var J=w(n);p=V(J,"Email"),J.forEach(m),g=N(D),k=T(D,"TH",{});var K=w(k);P=V(K,"Role"),K.forEach(m),b=N(D),E=T(D,"TH",{});var M=w(E);q=V(M,"View"),M.forEach(m),D.forEach(m),h.forEach(m),H=N($),c=T($,"TBODY",{class:!0});var U=w(c);for(let z=0;z<_.length;z+=1)_[z].l(U);U.forEach(m),$.forEach(m),this.h()},h(){L(t,"class","table__head"),L(c,"class","table__body"),L(a,"class","table")},m(s,$){r&&r.m(s,$),S(s,e,$),S(s,a,$),i(a,t),i(t,l),i(l,f),i(f,y),i(l,u),i(l,n),i(n,p),i(l,g),i(l,k),i(k,P),i(l,b),i(l,E),i(E,q),i(a,H),i(a,c);for(let h=0;h<_.length;h+=1)_[h].m(c,null);d=!0},p(s,$){if(s[2]?r?$&4&&O(r,1):(r=te(),r.c(),O(r,1),r.m(e.parentNode,e)):r&&(ie(),Q(r,1,1,()=>{r=null}),fe()),$&40){A=s[3];let h;for(h=0;h<A.length;h+=1){const D=ee(s,A,h);_[h]?_[h].p(D,$):(_[h]=le(D),_[h].c(),_[h].m(c,null))}for(;h<_.length;h+=1)_[h].d(1);_.length=A.length}},i(s){d||(O(r),d=!0)},o(s){Q(r),d=!1},d(s){r&&r.d(s),s&&m(e),s&&m(a),ce(_,s)}}}function ge(o){let e,a,t,l;function f(n){o[7](n)}function y(n){o[8](n)}let u={more:o[4]};return o[0]!==void 0&&(u.count=o[0]),o[1]!==void 0&&(u.offset=o[1]),e=new me({props:u}),W.push(()=>X(e,"count",f)),W.push(()=>X(e,"offset",y)),e.$on("refresh",o[6]),{c(){G(e.$$.fragment)},l(n){I(e.$$.fragment,n)},m(n,p){Y(e,n,p),l=!0},p(n,p){const g={};p&16&&(g.more=n[4]),!a&&p&1&&(a=!0,g.count=n[0],Z(()=>a=!1)),!t&&p&2&&(t=!0,g.offset=n[1],Z(()=>t=!1)),e.$set(g)},i(n){l||(O(e.$$.fragment,n),l=!0)},o(n){Q(e.$$.fragment,n),l=!1},d(n){j(e,n)}}}function be(o){let e,a;return e=new he({props:{title:"Players",text:"Below is a list of player accounts on this server",$$slots:{heading:[ge],default:[pe]},$$scope:{ctx:o}}}),{c(){G(e.$$.fragment)},l(t){I(e.$$.fragment,t)},m(t,l){Y(e,t,l),a=!0},p(t,[l]){const f={};l&16447&&(f.$$scope={dirty:l,ctx:t}),e.$set(f)},i(t){a||(O(e.$$.fragment,t),a=!0)},o(t){Q(e.$$.fragment,t),a=!1},d(t){j(e,t)}}}function $e(o,e,a){let t;se(o,_e,b=>a(5,t=b));let l=!0,f=[],y=!1,u=20,n=0;async function p(b,E){a(2,l=!0);try{let q=await ue(b,E);a(3,f=q.players),a(4,y=q.more)}catch(q){let H=q;console.error(H),H.text}finally{a(2,l=!1)}}function g(){p(n,u)}function k(b){u=b,a(0,u)}function P(b){n=b,a(1,n)}return o.$$.update=()=>{o.$$.dirty&3&&p(n,u)},[u,n,l,f,y,t,g,k,P]}class ke extends re{constructor(e){super(),ne(this,e,$e,be,oe,{})}}export{ke as default};